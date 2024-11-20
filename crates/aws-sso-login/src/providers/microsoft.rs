use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Input, Password};
use headless_chrome::Tab;
use log::{debug, trace};
use std::{sync::Arc, time::Duration};

pub(crate) fn page_router(tab: Arc<Tab>) -> Result<()> {
    let mut done = false;

    loop {
        tab.wait_until_navigated()?;

        if done && tab.get_title()? == "AWS access portal" {
            return Ok(());
        }

        if let Ok(elem_mfa_title) = tab.find_element("div#idDiv_SAOTCAS_Title.row.text-title") {
            let title = elem_mfa_title.get_inner_text()?;
            if title == "Approve sign in request" {
                mfa(tab.clone())?;
                remember(tab.clone())?;

                done = true;
            }
        } else if let Ok(elem_login_header) =
            tab.find_element("div#loginHeader.row.title.ext-title")
        {
            let page_type = elem_login_header.get_inner_text()?;

            match page_type.as_str() {
                "Sign in" => email(tab.clone())?,
                "Enter password" => password(tab.clone())?,
                _ => debug!("Unknown: {}", &page_type),
            }
        }
    }
}

fn email(tab: Arc<Tab>) -> Result<()> {
    trace!("EMAIL");

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

fn password(tab: Arc<Tab>) -> Result<()> {
    trace!("PASSWORD");

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

fn mfa(tab: Arc<Tab>) -> Result<()> {
    trace!("MFA");

    debug!("Waiting for MFA code");
    let mfa_code = tab
        .wait_for_element("div#idRichContext_DisplaySign.displaySign.display-sign-height")?
        .get_inner_text()?;
    println!("MFA: {mfa_code}");

    Ok(())
}

fn remember(tab: Arc<Tab>) -> Result<()> {
    trace!("REMEMBER");

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
