use crate::indexer::walker::FileEntry;
use crate::query::ast::{ActiveFilterDto, FileKind, Filter, Mode, Query, RangeOp, Scope, Term, TermField};
use crate::query::lexer::{tokenize, Token};
use crate::query::scope_trie::{DirectoryTrie, TrieMatchResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn parse_query(
    raw: &str,
    entries: &[FileEntry],
    path_index: &HashMap<PathBuf, usize>,
    trie: Option<&DirectoryTrie>,
    generation: u64,
) -> Query {
    let now_sec = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let tokens = tokenize(raw);
    let mut terms = Vec::new();
    let mut filters = Vec::new();
    let mut scopes = Vec::new();
    let mut hints = Vec::new();
    let mut active_filters_dto = Vec::new();
    let mut mode = Mode::Ranked;

    let mut is_first_token = true;

    for token in tokens {
        match token {
            Token::Word(ref text) if is_first_token && is_directory_like(text) && trie.is_some() => {
                // Leading directory-looking token (e.g. src/ or ~/Projects/)
                if let Some(dir_trie) = trie {
                    match dir_trie.resolve(text, entries, path_index) {
                        TrieMatchResult::Single(resolved) => {
                            scopes.push(Scope {
                                dir_id: resolved.dir_idx as u64,
                                include: true,
                                path: resolved.path.clone(),
                            });
                            active_filters_dto.push(ActiveFilterDto {
                                raw: text.clone(),
                                filter_type: "in".to_string(),
                                value: resolved.path.to_string_lossy().to_string(),
                                negated: false,
                            });
                        }
                        TrieMatchResult::Ambiguous(candidates) => {
                            terms.push(Term {
                                text: text.clone(),
                                field: TermField::Any,
                                phrase: false,
                                is_regex: false,
                            });
                            for cand in candidates.iter().take(2) {
                                hints.push(format!("did you mean 📁 {}?", cand.path.to_string_lossy()));
                            }
                        }
                        TrieMatchResult::None => {
                            terms.push(Term {
                                text: text.clone(),
                                field: TermField::Any,
                                phrase: false,
                                is_regex: false,
                            });
                        }
                    }
                }
            }
            Token::Word(text) => {
                terms.push(Term {
                    text,
                    field: TermField::Any,
                    phrase: false,
                    is_regex: false,
                });
            }
            Token::Quoted(phrase) => {
                terms.push(Term {
                    text: phrase,
                    field: TermField::Any,
                    phrase: true,
                    is_regex: false,
                });
            }
            Token::Regex(pattern) => {
                terms.push(Term {
                    text: pattern,
                    field: TermField::Any,
                    phrase: false,
                    is_regex: true,
                });
            }
            Token::NegatedWord(word) => {
                filters.push(Filter::Not(Box::new(Filter::Path(word.clone()))));
                active_filters_dto.push(ActiveFilterDto {
                    raw: format!("-{}", word),
                    filter_type: "exclude".to_string(),
                    value: word,
                    negated: true,
                });
            }
            Token::NegatedQuoted(phrase) => {
                filters.push(Filter::Not(Box::new(Filter::Name(phrase.clone()))));
                active_filters_dto.push(ActiveFilterDto {
                    raw: format!("-\"{}\"", phrase),
                    filter_type: "exclude".to_string(),
                    value: phrase,
                    negated: true,
                });
            }
            Token::KeyValue {
                negated,
                key,
                value,
                quoted_value,
            } => {
                let key_lower = key.to_lowercase();
                let raw_token = if negated {
                    if quoted_value {
                        format!("-{}:\"{}\"", key, value)
                    } else {
                        format!("-{}:{}", key, value)
                    }
                } else if quoted_value {
                    format!("{}:\"{}\"", key, value)
                } else {
                    format!("{}:{}", key, value)
                };

                match key_lower.as_str() {
                    "type" => {
                        let filter = Filter::Type(value.to_lowercase());
                        if negated {
                            filters.push(Filter::Not(Box::new(filter)));
                        } else {
                            filters.push(filter);
                        }
                        active_filters_dto.push(ActiveFilterDto {
                            raw: raw_token,
                            filter_type: "type".to_string(),
                            value,
                            negated,
                        });
                    }
                    "ext" => {
                        let ext_clean = value.trim_start_matches('.').to_lowercase();
                        let filter = Filter::Ext(ext_clean.clone());
                        if negated {
                            filters.push(Filter::Not(Box::new(filter)));
                        } else {
                            filters.push(filter);
                        }
                        active_filters_dto.push(ActiveFilterDto {
                            raw: raw_token,
                            filter_type: "ext".to_string(),
                            value: ext_clean,
                            negated,
                        });
                    }
                    "in" => {
                        let mut resolved_scope = false;
                        if let Some(dir_trie) = trie {
                            match dir_trie.resolve(&value, entries, path_index) {
                                TrieMatchResult::Single(resolved) => {
                                    scopes.push(Scope {
                                        dir_id: resolved.dir_idx as u64,
                                        include: !negated,
                                        path: resolved.path.clone(),
                                    });
                                    active_filters_dto.push(ActiveFilterDto {
                                        raw: raw_token.clone(),
                                        filter_type: "in".to_string(),
                                        value: resolved.path.to_string_lossy().to_string(),
                                        negated,
                                    });
                                    resolved_scope = true;
                                }
                                TrieMatchResult::Ambiguous(candidates) => {
                                    terms.push(Term {
                                        text: value.clone(),
                                        field: TermField::Path,
                                        phrase: false,
                                        is_regex: false,
                                    });
                                    for cand in candidates.iter().take(2) {
                                        hints.push(format!("did you mean 📁 {}?", cand.path.to_string_lossy()));
                                    }
                                    resolved_scope = true;
                                }
                                TrieMatchResult::None => {}
                            }
                        }
                        if !resolved_scope {
                            // If trie could not resolve, match path as filter
                            let filter = Filter::Path(value.clone());
                            if negated {
                                filters.push(Filter::Not(Box::new(filter)));
                            } else {
                                filters.push(filter);
                            }
                            active_filters_dto.push(ActiveFilterDto {
                                raw: raw_token,
                                filter_type: "in".to_string(),
                                value,
                                negated,
                            });
                        }
                    }
                    "path" => {
                        terms.push(Term {
                            text: value.clone(),
                            field: TermField::Path,
                            phrase: quoted_value,
                            is_regex: false,
                        });
                        active_filters_dto.push(ActiveFilterDto {
                            raw: raw_token,
                            filter_type: "path".to_string(),
                            value,
                            negated,
                        });
                    }
                    "modified" => {
                        if let Some(range_op) = parse_modified_range(&value, now_sec) {
                            let filter = Filter::Modified(range_op);
                            if negated {
                                filters.push(Filter::Not(Box::new(filter)));
                            } else {
                                filters.push(filter);
                            }
                            active_filters_dto.push(ActiveFilterDto {
                                raw: raw_token,
                                filter_type: "modified".to_string(),
                                value,
                                negated,
                            });
                        } else {
                            hints.push(format!("Invalid modified filter '{}'; use <7d, >30d, 7d..30d", value));
                            terms.push(Term {
                                text: raw_token,
                                field: TermField::Any,
                                phrase: false,
                                is_regex: false,
                            });
                        }
                    }
                    "size" => {
                        if let Some(range_op) = parse_size_range(&value) {
                            let filter = Filter::Size(range_op);
                            if negated {
                                filters.push(Filter::Not(Box::new(filter)));
                            } else {
                                filters.push(filter);
                            }
                            active_filters_dto.push(ActiveFilterDto {
                                raw: raw_token,
                                filter_type: "size".to_string(),
                                value,
                                negated,
                            });
                        } else {
                            hints.push(format!("Invalid size filter '{}'; use >2mb, <100kb, 1mb..10mb", value));
                            terms.push(Term {
                                text: raw_token,
                                field: TermField::Any,
                                phrase: false,
                                is_regex: false,
                            });
                        }
                    }
                    "is" => {
                        let filter = match value.to_lowercase().as_str() {
                            "dir" | "directory" | "folder" => Some(Filter::Kind(FileKind::Dir)),
                            "file" => Some(Filter::Kind(FileKind::File)),
                            "image" | "video" | "audio" | "doc" | "code" | "archive" => {
                                Some(Filter::Type(value.to_lowercase()))
                            }
                            _ => None,
                        };

                        if let Some(f) = filter {
                            if negated {
                                filters.push(Filter::Not(Box::new(f)));
                            } else {
                                filters.push(f);
                            }
                            active_filters_dto.push(ActiveFilterDto {
                                raw: raw_token,
                                filter_type: "is".to_string(),
                                value,
                                negated,
                            });
                        } else {
                            hints.push(format!("Unknown 'is:{}' filter; use is:image, is:dir, is:file", value));
                            terms.push(Term {
                                text: raw_token,
                                field: TermField::Any,
                                phrase: false,
                                is_regex: false,
                            });
                        }
                    }
                    "lang" => {
                        let filter = Filter::Lang(value.to_lowercase());
                        if negated {
                            filters.push(Filter::Not(Box::new(filter)));
                        } else {
                            filters.push(filter);
                        }
                        active_filters_dto.push(ActiveFilterDto {
                            raw: raw_token,
                            filter_type: "lang".to_string(),
                            value,
                            negated,
                        });
                    }
                    "name" => {
                        terms.push(Term {
                            text: value.clone(),
                            field: TermField::Name,
                            phrase: quoted_value,
                            is_regex: false,
                        });
                    }
                    "content" => {
                        terms.push(Term {
                            text: value.clone(),
                            field: TermField::Content,
                            phrase: quoted_value,
                            is_regex: false,
                        });
                    }
                    "mode" => {
                        if value.eq_ignore_ascii_case("exhaustive") {
                            mode = Mode::Exhaustive;
                        } else {
                            mode = Mode::Ranked;
                        }
                    }
                    _ => {
                        // Unknown key -> treat as literal text (never zero results)
                        terms.push(Term {
                            text: raw_token,
                            field: TermField::Any,
                            phrase: false,
                            is_regex: false,
                        });
                    }
                }
            }
        }
        is_first_token = false;
    }

    let _ = generation; // Used for reference

    Query {
        terms,
        filters,
        scopes,
        mode,
        raw: raw.to_string(),
        hints,
        active_filters_dto,
    }
}

fn is_directory_like(s: &str) -> bool {
    s.ends_with('/') || s.ends_with('\\') || s.starts_with("~/") || s.starts_with("./")
}

fn parse_duration_seconds(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    if num_str.is_empty() {
        return None;
    }
    let num: f64 = num_str.parse().ok()?;
    let suffix = s[num_str.len()..].trim();

    let secs = match suffix {
        "" | "s" | "sec" | "secs" => num,
        "m" | "min" | "mins" => num * 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => num * 3600.0,
        "d" | "day" | "days" => num * 86400.0,
        "w" | "wk" | "wks" | "week" | "weeks" => num * 7.0 * 86400.0,
        "y" | "yr" | "yrs" | "year" | "years" => num * 365.0 * 86400.0,
        _ => return None,
    };
    Some(secs as u64)
}

fn parse_modified_range(val: &str, now_sec: u64) -> Option<RangeOp> {
    let s = val.trim();
    if let Some((first, second)) = s.split_once("..") {
        let dur1 = parse_duration_seconds(first)?;
        let dur2 = parse_duration_seconds(second)?;
        let (min_dur, max_dur) = if dur1 <= dur2 { (dur1, dur2) } else { (dur2, dur1) };
        let low_time = now_sec.saturating_sub(max_dur);
        let high_time = now_sec.saturating_sub(min_dur);
        return Some(RangeOp::Between(low_time, high_time));
    }
    if let Some(rest) = s.strip_prefix('<') {
        let dur = parse_duration_seconds(rest.trim_start_matches('='))?;
        return Some(RangeOp::Gte(now_sec.saturating_sub(dur)));
    }
    if let Some(rest) = s.strip_prefix('>') {
        let dur = parse_duration_seconds(rest.trim_start_matches('='))?;
        return Some(RangeOp::Lte(now_sec.saturating_sub(dur)));
    }
    let dur = parse_duration_seconds(s)?;
    Some(RangeOp::Gte(now_sec.saturating_sub(dur)))
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    if num_str.is_empty() {
        return None;
    }
    let num: f64 = num_str.parse().ok()?;
    let suffix = s[num_str.len()..].trim();

    let bytes = match suffix {
        "" | "b" => num,
        "k" | "kb" | "kib" => num * 1024.0,
        "m" | "mb" | "mib" => num * 1024.0 * 1024.0,
        "g" | "gb" | "gib" => num * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(bytes as u64)
}

fn parse_size_range(val: &str) -> Option<RangeOp> {
    let s = val.trim();
    if let Some((low_s, high_s)) = s.split_once("..") {
        let low = parse_size(low_s)?;
        let high = parse_size(high_s)?;
        return Some(RangeOp::Between(low, high));
    }
    if let Some(rest) = s.strip_prefix(">=") {
        return Some(RangeOp::Gte(parse_size(rest)?));
    }
    if let Some(rest) = s.strip_prefix('>') {
        return Some(RangeOp::Gt(parse_size(rest)?));
    }
    if let Some(rest) = s.strip_prefix("<=") {
        return Some(RangeOp::Lte(parse_size(rest)?));
    }
    if let Some(rest) = s.strip_prefix('<') {
        return Some(RangeOp::Lt(parse_size(rest)?));
    }
    let size = parse_size(s)?;
    Some(RangeOp::Between(size.saturating_sub(1024), size + 1024))
}
