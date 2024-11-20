pub(crate) fn init() {
    env_logger::Builder::from_default_env()
        .filter_module("headless_chrome", log::LevelFilter::Off)
        .filter_module("tungstenite", log::LevelFilter::Off)
        .init();
}
