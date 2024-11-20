pub(crate) mod aws;
pub(crate) mod cli;
pub(crate) mod logging;
pub(crate) mod providers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init();
    let args = cli::Args::new();

    if let Some(profile) = args.profile {
        aws::login_profile(&profile, args.gui).await
    } else {
        aws::login_profile_select(&args).await
    }
}
