mod lexicon;
mod output;
mod pairs;
mod sources;

use std::error::Error;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (inputs, archive) = match &args[..] {
        [] => (
            PathBuf::from("inputs"),
            PathBuf::from("dist/mandarin-collocations.zip"),
        ),
        [inputs, archive] => (PathBuf::from(inputs), PathBuf::from(archive)),
        _ => return Err("usage: mandarin-collocations [INPUT_DIRECTORY OUTPUT_ZIP]".into()),
    };
    let mut lexicon = lexicon::Lexicon::load(&inputs.join("cedict.txt.gz"))?;
    let mut pairs = pairs::Pairs::default();
    sources::load(&inputs, &mut lexicon, &mut pairs)?;
    let styles = include_str!("../styles.css");
    let total_pairs = pairs.len();
    let entries = pairs.rank(&lexicon);
    let report = output::write(&archive, styles, &lexicon, &entries, total_pairs)?;
    eprintln!(
        "Wrote {} lookup forms for {} headwords to {}",
        report.forms,
        report.headwords,
        archive.display()
    );
    Ok(())
}

fn open(path: &Path) -> Result<std::fs::File> {
    std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()).into())
}
