use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectTool {
    Claude,
    Cursor,
    Copilot,
    Gemini,
    Windsurf,
    Aider,
}

impl std::fmt::Display for ProjectTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ProjectTool::Claude => "Claude",
            ProjectTool::Cursor => "Cursor",
            ProjectTool::Copilot => "Copilot",
            ProjectTool::Gemini => "Gemini",
            ProjectTool::Windsurf => "Windsurf",
            ProjectTool::Aider => "Aider",
        })
    }
}

#[derive(Debug, Clone)]
pub struct DetectedTool {
    pub tool: ProjectTool,
    pub config_path: PathBuf,
}

const CONFIG_FILES: &[(&str, ProjectTool)] = &[
    ("CLAUDE.md", ProjectTool::Claude),
    ("AGENTS.md", ProjectTool::Claude),
    (".cursorrules", ProjectTool::Cursor),
    (".github/copilot-instructions.md", ProjectTool::Copilot),
    ("GEMINI.md", ProjectTool::Gemini),
    (".windsurfrules", ProjectTool::Windsurf),
];

const CONFIG_DIRS: &[(&str, ProjectTool)] = &[
    (".cursor/rules", ProjectTool::Cursor),
];

pub fn detect_project_tools(dir: &Path) -> Vec<DetectedTool> {
    let mut detected = Vec::new();

    for &(file, tool) in CONFIG_FILES {
        let path = dir.join(file);
        if path.is_file() {
            detected.push(DetectedTool {
                tool,
                config_path: path,
            });
        }
    }

    for &(subdir, tool) in CONFIG_DIRS {
        let path = dir.join(subdir);
        if path.is_dir() {
            detected.push(DetectedTool {
                tool,
                config_path: path,
            });
        }
    }

    // .aider* — any file starting with ".aider" in the project root
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(".aider") && entry.path().is_file() {
                    detected.push(DetectedTool {
                        tool: ProjectTool::Aider,
                        config_path: entry.path(),
                    });
                    break;
                }
            }
        }
    }

    detected.sort_by(|a, b| a.tool.to_string().cmp(&b.tool.to_string()));
    detected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_claude_md() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Claude").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Claude));
    }

    #[test]
    fn detects_agents_md() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Claude));
    }

    #[test]
    fn detects_cursorrules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "{}").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Cursor));
    }

    #[test]
    fn detects_cursor_rules_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cursor_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&cursor_dir).unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Cursor));
    }

    #[test]
    fn detects_copilot_instructions() {
        let dir = tempfile::tempdir().unwrap();
        let gh = dir.path().join(".github");
        std::fs::create_dir(&gh).unwrap();
        std::fs::write(gh.join("copilot-instructions.md"), "# Copilot").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Copilot));
    }

    #[test]
    fn detects_gemini_md() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("GEMINI.md"), "# Gemini").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Gemini));
    }

    #[test]
    fn detects_windsurfrules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".windsurfrules"), "{}").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Windsurf));
    }

    #[test]
    fn detects_aider_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".aider.conf.yml"), "model: gpt-4").unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.iter().any(|t| t.tool == ProjectTool::Aider));
    }

    #[test]
    fn empty_dir_detects_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let tools = detect_project_tools(dir.path());
        assert!(tools.is_empty());
    }

    #[test]
    fn detects_multiple_tools() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "").unwrap();
        std::fs::write(dir.path().join("GEMINI.md"), "").unwrap();
        std::fs::write(dir.path().join(".cursorrules"), "").unwrap();
        let tools = detect_project_tools(dir.path());
        assert_eq!(tools.len(), 3);
    }
}
