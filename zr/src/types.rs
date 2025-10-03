use camino::Utf8PathBuf;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub snapname: String,
    pub path: Utf8PathBuf,
    pub size: u64,
    pub mtime: i64,
}

pub type Candidates = Vec<Candidate>;
pub type CopyAction = Option<(Utf8PathBuf, Utf8PathBuf)>;
pub type UserChoice = Option<(usize, Option<String>)>;
