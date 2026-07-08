use facet::Facet;
use figue as args;
use figue::FigueBuiltins;

#[derive(Facet, Debug)]
pub struct Cli {
    /// Enable debug logging.
    #[facet(args::named, default)]
    pub debug: bool,

    #[facet(flatten)]
    pub builtins: FigueBuiltins,

    #[facet(args::subcommand)]
    pub command: Option<Command>,
}

#[derive(Facet, Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum Command {
    /// Launch the graphical cache explorer.
    Gui(GuiArgs),

    /// List cache files and detected media kinds.
    List(ListArgs),

    /// Inspect one cache file.
    Inspect(InspectArgs),

    /// Extract a PNG thumbnail from a video cache file.
    Thumbnail(ThumbnailArgs),
}

#[derive(Facet, Default, Clone, PartialEq, Debug)]
pub struct GuiArgs {
    /// Directory to scan. Defaults to %APPDATA%\Discord\Cache\Cache_Data.
    #[facet(args::named)]
    pub cache_dir: Option<String>,
}

#[derive(Facet, Clone, PartialEq, Debug)]
pub struct ListArgs {
    /// Directory to scan. Defaults to %APPDATA%\Discord\Cache\Cache_Data.
    #[facet(args::named)]
    pub cache_dir: Option<String>,

    /// Maximum number of files to print.
    #[facet(args::named, default = 50)]
    pub limit: usize,
}

#[derive(Facet, Clone, PartialEq, Debug)]
pub struct InspectArgs {
    /// Cache file to inspect.
    #[facet(args::positional)]
    pub path: String,
}

#[derive(Facet, Clone, PartialEq, Debug)]
pub struct ThumbnailArgs {
    /// Video cache file to decode.
    #[facet(args::positional)]
    pub input: String,

    /// Output PNG path. Defaults to a temp thumbnail path.
    #[facet(args::named)]
    pub output: Option<String>,
}

impl Default for Command {
    fn default() -> Self {
        Self::Gui(GuiArgs::default())
    }
}

impl Cli {
    /// # Errors
    ///
    /// Returns an error if the selected command fails.
    pub fn invoke(self) -> eyre::Result<()> {
        self.command.unwrap_or_default().invoke()
    }
}

impl Command {
    /// # Errors
    ///
    /// Returns an error if the command fails.
    pub fn invoke(self) -> eyre::Result<()> {
        match self {
            Self::Gui(args) => args.invoke(),
            Self::List(args) => args.invoke(),
            Self::Inspect(args) => args.invoke(),
            Self::Thumbnail(args) => args.invoke(),
        }
    }
}

impl GuiArgs {
    /// # Errors
    ///
    /// Returns an error if the GUI fails to start.
    pub fn invoke(self) -> eyre::Result<()> {
        crate::gui::run_gui(self.cache_dir.map(Into::into))
    }
}

impl ListArgs {
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be scanned.
    pub fn invoke(self) -> eyre::Result<()> {
        let cache_dir = match self.cache_dir {
            Some(path) => path.into(),
            None => crate::cache::default_cache_dir()?,
        };
        let entries = crate::cache::scan_cache_dir(&cache_dir)?;

        println!("cache_dir={}", cache_dir.display());
        println!("files={}", entries.len());
        for (index, entry) in entries.iter().take(self.limit).enumerate() {
            println!(
                "{index:>4} {:>5} {:>10} {}",
                entry.kind.label(),
                entry.len,
                entry.file_name
            );
        }
        Ok(())
    }
}

impl InspectArgs {
    /// # Errors
    ///
    /// Returns an error if the file cannot be inspected.
    pub fn invoke(self) -> eyre::Result<()> {
        let path = std::path::PathBuf::from(self.path);
        let metadata = std::fs::metadata(&path)?;
        let kind = crate::cache::sniff_kind(&path);

        println!("path={}", path.display());
        println!("size={}", metadata.len());
        println!("kind={}", kind.label());

        match kind {
            crate::cache::CacheKind::Image => match crate::cache::decode_image(&path) {
                Ok(image) => {
                    println!("image_decode=ok");
                    println!("image_width={}", image.width());
                    println!("image_height={}", image.height());
                }
                Err(error) => {
                    println!("image_decode=error");
                    println!("image_error={error}");
                }
            },
            crate::cache::CacheKind::Video => match crate::video::probe(&path) {
                Ok(output) => {
                    println!("ffprobe=ok");
                    print!("{output}");
                }
                Err(error) => {
                    println!("ffprobe=error");
                    println!("ffprobe_error={error}");
                }
            },
            crate::cache::CacheKind::Other => {}
        }

        Ok(())
    }
}

impl ThumbnailArgs {
    /// # Errors
    ///
    /// Returns an error if thumbnail extraction fails.
    pub fn invoke(self) -> eyre::Result<()> {
        let input = std::path::PathBuf::from(self.input);
        let output = self
            .output
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| crate::video::thumbnail_path_for(&input));
        crate::video::extract_thumbnail(&input, &output)?;
        println!("{}", output.display());
        Ok(())
    }
}
