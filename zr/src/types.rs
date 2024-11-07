use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub snapname: String,
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

pub type Candidates = Vec<Candidate>;
pub type CopyAction = Option<(PathBuf, PathBuf)>;
pub type UserChoice = Option<(usize, Option<String>)>;
