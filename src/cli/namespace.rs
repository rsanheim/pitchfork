use crate::Result;
use crate::env;
use crate::pitchfork_toml::PitchforkToml;

/// Print the namespace for the current directory
#[derive(Debug, clap::Args)]
#[clap(
    visible_alias = "ns",
    verbatim_doc_comment,
    long_about = "\
Print the namespace for the current directory

Resolves the nearest pitchfork config file and prints the namespace its
daemons belong to. Prints \"global\" when no project config applies.

With namespace_per_worktree enabled, the output includes the per-worktree
hash suffix, so it identifies the current checkout.

Example:
  pitchfork namespace
  pitchfork stop \"$(pitchfork namespace)/api\""
)]
pub struct Namespace {}

impl Namespace {
    pub async fn run(&self) -> Result<()> {
        let ns = PitchforkToml::namespace_for_dir(&env::CWD)?;
        println!("{ns}");
        Ok(())
    }
}
