use anyhow::{anyhow, bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use directories::UserDirs;
use headless_chrome::protocol::cdp::Target::CreateTarget;
use headless_chrome::{Browser, LaunchOptions};
use log::debug;
use log::error;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::cli::Args;
use crate::providers::microsoft;

pub(crate) fn aws_config_path() -> Result<PathBuf> {
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

pub(crate) async fn login_profile(profile: &str, gui: bool) -> Result<()> {
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

pub(crate) async fn login_profile_select(args: &Args) -> Result<()> {
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
    let config_content = tokio::fs::read_to_string(aws_config_path).await?;
    let config: HashMap<String, HashMap<String, String>> = serde_ini::from_str(&config_content)?;
    let sso_profiles = config
        .keys()
        .filter(|key| key.starts_with("sso-session"))
        .filter_map(|key| key.split_whitespace().nth(1).map(String::from))
        .collect::<Vec<_>>();

    Ok(sso_profiles)
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

    microsoft::page_router(tab.clone())?;

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
