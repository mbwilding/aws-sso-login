use anyhow::{anyhow, bail, Result};
use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password, Select};
use directories::UserDirs;
use headless_chrome::protocol::cdp::Target::CreateTarget;
use headless_chrome::{Browser, LaunchOptions, Tab};
use log::{debug, error};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Expose the browser
    #[arg(short, long, default_value = "false")]
    pub gui: bool,

    /// AWS SSO profile
    #[arg(short, long)]
    pub profile: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();
    let args = Args::parse();

    if let Some(profile) = args.profile {
        return login_profile(&profile, args.gui).await;
    } else {
        return select_profile(&args).await;
    }
}

fn init_logging() {
    env_logger::Builder::from_default_env()
        .filter_module("headless_chrome", log::LevelFilter::Off)
        .filter_module("tungstenite", log::LevelFilter::Off)
        .init();
}

fn aws_config_path() -> Result<PathBuf> {
    let user_dirs = UserDirs::new().ok_or_else(|| {
        let err = "Unable to find the user directory";
        error!("{err}");
        anyhow!(err)
    })?;
    let config_path = user_dirs.home_dir().join(".aws/config");
    if config_path.exists() {
        Ok(config_path)
    } else {
        let err = "AWS config file not found, please run aws cli with -c or --configure";
        error!("{err}");
        bail!(err)
    }
}

async fn select_profile(args: &Args) -> Result<()> {
    let sso_profiles = aws_sso_profiles().await?;

    if sso_profiles.len() == 1 {
        login_profile(&sso_profiles[0], args.gui).await?;
    } else {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("SSO")
            .interact()?;

        login_profile(&sso_profiles[selection], args.gui).await?;
    }

    Ok(())
}

async fn aws_sso_profiles() -> Result<Vec<String>> {
    let aws_config_path = aws_config_path()?;
    let config_content = fs::read_to_string(aws_config_path).await?;
    let config: HashMap<String, HashMap<String, String>> = serde_ini::from_str(&config_content)?;
    let sso_profiles = config
        .keys()
        .filter(|key| key.starts_with("sso-session"))
        .filter_map(|key| key.split_whitespace().nth(1).map(String::from))
        .collect::<Vec<_>>();

    Ok(sso_profiles)
}

async fn login_profile(profile: &str, gui: bool) -> Result<()> {
    println!("Login: {}", profile);

    let proc = Command::new("aws")
        .args(["sso", "login", "--no-browser", "--sso-session", profile])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute command");

    let stdout = BufReader::new(proc.stdout.expect("Failed to capture stdout"));

    let mut lines = stdout.lines();
    while let Some(line) = lines.next_line().await? {
        if line.contains("user_code") {
            browser_login(line, gui)?;
        }
    }

    Ok(())
}

fn browser_login(url: String, gui: bool) -> Result<()> {
    let width = 425;
    let height = 550;

    let user_data_path = match UserDirs::new() {
        Some(user_dirs) => user_dirs.home_dir().join(".aws-sso-login"),
        None => Err(anyhow!("Unable to get user directories"))?,
    };

    let options = LaunchOptions::default_builder()
        .headless(!gui)
        .window_size(Some((width, height)))
        .sandbox(false)
        .user_data_dir(Some(user_data_path))
        .build()?;

    let browser = Browser::new(options)?;

    debug!("Url: {}", &url);
    let tab = browser.new_tab_with_options(CreateTarget {
        url,
        width: Some(width - 15),
        height: Some(height - 35),
        browser_context_id: None,
        enable_begin_frame_control: None,
        new_window: Some(false),
        background: None,
    })?;

    debug!("Clicking on CLI verification button");
    tab.wait_for_element("#cli_verification_btn")?.click()?;

    page_router_microsoft(tab.clone())?;

    debug!("Waiting and clicking on continue");
    tab.wait_for_element(
        "button.awsui_button_vjswe_1dg71_153.awsui_variant-primary_vjswe_1dg71_296",
    )?
    .click()?;

    debug!("Waiting on confirmation");
    let result = tab
        .wait_for_element("div.awsui_header_mx3cw_4ej0u_321.awsui_header_17427_1ns0c_5")?
        .get_inner_text()?;
    println!("Status: {result}");

    Ok(())
}

fn page_router_microsoft(tab: Arc<Tab>) -> Result<()> {
    let mut done = false;

    loop {
        tab.wait_until_navigated()?;

        if done && tab.get_title()? == "AWS access portal" {
            return Ok(());
        }

        if let Ok(elem_mfa_title) = tab.find_element("div#idDiv_SAOTCAS_Title.row.text-title") {
            let title = elem_mfa_title.get_inner_text()?;
            if title == "Approve sign in request" {
                mfa_microsoft(tab.clone())?;
                remember_microsoft(tab.clone())?;

                done = true;
            }
        } else if let Ok(elem_login_header) =
            tab.find_element("div#loginHeader.row.title.ext-title")
        {
            let page_type = elem_login_header.get_inner_text()?;

            match page_type.as_str() {
                "Sign in" => email_microsoft(tab.clone())?,
                "Enter password" => password_microsoft(tab.clone())?,
                _ => debug!("Unknown: {}", &page_type),
            }
        }
    }
}

fn email_microsoft(tab: Arc<Tab>) -> Result<()> {
    debug!("EMAIL");

    debug!("Waiting and clicking on email input");
    let input = tab.wait_for_element(
        "input#i0116.form-control.ltr_override.input.ext-input.text-box.ext-text-box",
    )?;

    debug!("Clicking email input");
    input.click()?;

    debug!("Clearing email input");
    input.call_js_fn("function() { this.value = ''; }", Vec::new(), false)?;

    debug!("Asking email input");
    let email: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Email")
        .interact_text()?;

    debug!("Entering email");
    tab.send_character(&email)?;

    debug!("Pressing enter");
    tab.press_key("Enter")?;

    Ok(())
}

fn password_microsoft(tab: Arc<Tab>) -> Result<()> {
    debug!("PASSWORD");

    debug!("Waiting and clicking on password input");
    let input =
        tab.wait_for_element("input#i0118.form-control.input.ext-input.text-box.ext-text-box")?;

    debug!("Clicking password input");
    input.click()?;

    debug!("Clearing password input");
    input.call_js_fn("function() { this.value = ''; }", Vec::new(), false)?;

    debug!("Asking password input");
    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Password")
        .interact()?;

    debug!("Entering password");
    tab.send_character(&password)?;

    debug!("Pressing enter");
    tab.press_key("Enter")?;

    Ok(())
}

fn mfa_microsoft(tab: Arc<Tab>) -> Result<()> {
    debug!("MFA");

    debug!("Waiting for MFA code");
    let mfa_code = tab
        .wait_for_element("div#idRichContext_DisplaySign.displaySign.display-sign-height")?
        .get_inner_text()?;
    println!("MFA code: {mfa_code}");

    Ok(())
}

fn remember_microsoft(tab: Arc<Tab>) -> Result<()> {
    debug!("REMEMBER");

    debug!("Waiting and clicking on don't ask again");
    tab.wait_for_element_with_custom_timeout(
        "input#KmsiCheckboxField",
        Duration::from_secs(3 * 60),
    )?
    .click()?;

    debug!("Waiting and clicking on confirmation");
    tab.wait_for_element(
        "input#idSIButton9.win-button.button_primary.button.ext-button.primary.ext-primary",
    )?
    .click()?;

    Ok(())
}
