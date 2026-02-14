use std::collections::VecDeque;
use std::path::PathBuf;

pub const MAX_FEN_HISTORY: usize = 20;
pub const STANDARD_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Debug, Clone)]
pub struct FenHistoryEntry {
    pub fen: String,
    pub label: Option<String>,
}

pub struct FenHistory {
    entries: VecDeque<FenHistoryEntry>,
}

impl FenHistory {
    pub fn new() -> Self {
        let mut entries = VecDeque::new();
        // Always include standard position as first entry
        entries.push_back(FenHistoryEntry {
            fen: STANDARD_FEN.to_string(),
            label: Some("Standard Position".to_string()),
        });
        Self { entries }
    }

    pub fn add_fen(&mut self, fen: String) {
        // Remove if already exists (deduplicate)
        self.entries.retain(|entry| entry.fen != fen);

        // Add to front (after standard position)
        self.entries.insert(
            1,
            FenHistoryEntry {
                fen,
                label: None,
            },
        );

        // Cap at MAX_FEN_HISTORY (keep standard position + MAX-1)
        while self.entries.len() > MAX_FEN_HISTORY {
            self.entries.pop_back();
        }
    }

    pub fn entries(&self) -> &VecDeque<FenHistoryEntry> {
        &self.entries
    }

    pub fn load_from_file() -> Result<Self, std::io::Error> {
        let config_dir = get_config_dir();
        let file_path = config_dir.join("fen_history.txt");

        if !file_path.exists() {
            return Ok(Self::new());
        }

        let contents = std::fs::read_to_string(file_path)?;
        let mut entries = VecDeque::new();

        // Always add standard position first
        entries.push_back(FenHistoryEntry {
            fen: STANDARD_FEN.to_string(),
            label: Some("Standard Position".to_string()),
        });

        // Load saved FENs
        for line in contents.lines().take(MAX_FEN_HISTORY - 1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((fen, label)) = line.split_once('|') {
                entries.push_back(FenHistoryEntry {
                    fen: fen.to_string(),
                    label: Some(label.to_string()),
                });
            } else {
                entries.push_back(FenHistoryEntry {
                    fen: line.to_string(),
                    label: None,
                });
            }
        }

        Ok(Self { entries })
    }

    pub fn save_to_file(&self) -> Result<(), std::io::Error> {
        let config_dir = get_config_dir();
        std::fs::create_dir_all(&config_dir)?;

        let file_path = config_dir.join("fen_history.txt");
        let mut content = String::new();

        // Skip first entry (standard position, don't persist)
        for entry in self.entries.iter().skip(1) {
            if let Some(label) = &entry.label {
                content.push_str(&format!("{}|{}\n", entry.fen, label));
            } else {
                content.push_str(&format!("{}\n", entry.fen));
            }
        }

        std::fs::write(file_path, content)?;
        Ok(())
    }
}

impl Default for FenHistory {
    fn default() -> Self {
        Self::new()
    }
}

fn get_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("chesstty")
}
