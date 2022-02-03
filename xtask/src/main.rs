use structopt::StructOpt;
use xshell::cmd;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T> = std::result::Result<T, Error>;

/// Tooling for `yosh.is`
#[derive(StructOpt)]
enum Opt {
    /// Package up the extension.
    Package,
}

fn main() -> Result<()> {
    match Opt::from_args() {
        Opt::Package => package(),
    }
}

fn package() -> Result<()> {
    cmd!("npm install").run()?;
    cmd!("npm run compile").run()?;
    cmd!("npm run package").run()?;
    Ok(())
}
