#![windows_subsystem = "windows"]

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    discord_cache_explorer::main()
}
