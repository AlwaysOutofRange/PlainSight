use serde::Serialize;

const DEFAULT_MAX_CHUNK_LINES: usize = 120;
const DEFAULT_CHUNK_OVERLAP_LINES: usize = 20;
const DEFAULT_MAX_CHUNK_CHARS: usize = 6000;
const DEFAULT_MAX_CHUNK_TOKENS: usize = 1300;

#[derive(Debug, Clone, Copy)]
struct ChunkConfig {
    max_lines: usize,
    overlap_lines: usize,
    max_chars: usize,
    max_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceChunk {
    pub chunk_id: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceIndex {
    pub language: String,
    pub line_count: usize,
    pub chunk_count: usize,
    pub chunks: Vec<SourceChunk>,
}

pub fn build_source_index(source: &str, language: &str) -> SourceIndex {
    let config = chunk_config(language);
    let lines: Vec<&str> = source.lines().collect();
    let line_count = lines.len();

    if lines.is_empty() {
        return SourceIndex {
            language: language.to_string(),
            line_count: 0,
            chunk_count: 0,
            chunks: Vec::new(),
        };
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < lines.len() {
        let mut end = usize::min(start + config.max_lines, lines.len());

        // Bound long chunks by characters and estimated tokens so prompts stay predictable.
        while end > start {
            let segment = &lines[start..end];
            let char_len: usize = segment.iter().map(|l| l.len() + 1).sum();
            let token_estimate = estimate_tokens(segment);
            if char_len <= config.max_chars && token_estimate <= config.max_tokens {
                break;
            }
            end -= 1;
        }

        if end == start {
            end = usize::min(start + 1, lines.len());
        }

        let content = lines[start..end].join("\n");
        chunks.push(SourceChunk {
            chunk_id: chunks.len(),
            start_line: start + 1,
            end_line: end,
            content,
        });

        if end >= lines.len() {
            break;
        }

        let overlap = config.overlap_lines.min(end - start);
        start = end - overlap;
    }

    SourceIndex {
        language: language.to_string(),
        line_count,
        chunk_count: chunks.len(),
        chunks,
    }
}

fn chunk_config(language: &str) -> ChunkConfig {
    match language {
        "python" => ChunkConfig {
            max_lines: 100,
            overlap_lines: 14,
            max_chars: 5200,
            max_tokens: 1100,
        },
        "javascript" | "typescript" => ChunkConfig {
            max_lines: 110,
            overlap_lines: 18,
            max_chars: 5600,
            max_tokens: 1200,
        },
        "java" | "kotlin" | "csharp" => ChunkConfig {
            max_lines: 95,
            overlap_lines: 16,
            max_chars: 5400,
            max_tokens: 1150,
        },
        "c" | "cpp" => ChunkConfig {
            max_lines: 105,
            overlap_lines: 18,
            max_chars: 5600,
            max_tokens: 1200,
        },
        _ => ChunkConfig {
            max_lines: DEFAULT_MAX_CHUNK_LINES,
            overlap_lines: DEFAULT_CHUNK_OVERLAP_LINES,
            max_chars: DEFAULT_MAX_CHUNK_CHARS,
            max_tokens: DEFAULT_MAX_CHUNK_TOKENS,
        },
    }
}

fn estimate_tokens(lines: &[&str]) -> usize {
    let mut total = 0usize;
    for line in lines {
        // Practical, cheap approximation: ~4 chars/token with a floor from whitespace splits.
        let by_chars = line.chars().count().div_ceil(4);
        let by_words = line.split_whitespace().count();
        total += by_chars.max(by_words / 2 + 1);
    }
    total
}
