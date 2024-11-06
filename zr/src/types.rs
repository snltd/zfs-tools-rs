use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub snapname: String,
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

pub type Candidates = Vec<Candidate>;
pub type CandidatesResult = Result<Candidates, std::io::Error>;
pub type CopyAction = Option<(PathBuf, PathBuf)>;
pub type CopyActionResult = Result<CopyAction, std::io::Error>;
pub type IoResult<T> = Result<T, std::io::Error>;
pub type UserChoice = Option<(usize, Option<String>)>;
// pub type
