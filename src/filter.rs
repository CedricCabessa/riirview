use anyhow::Result;

#[derive(Debug)]
pub struct Filter {
    pub title: String,
    pub author: String,
    pub repo: String,
    pub state: String,
}

impl Filter {
    pub fn parse(input: &str) -> Result<Self> {
        let (filter_accumulator, title_parts_accumulator) = input.split_whitespace().fold(
            (
                // Initial Filter state (all empty strings)
                Filter {
                    title: String::new(),
                    author: String::new(),
                    repo: String::new(),
                    state: String::new(),
                },
                // Accumulator for title parts
                Vec::new(),
            ),
            |(mut filter, mut title_parts), word| {
                if word.starts_with("author:") {
                    filter.author = word.trim_start_matches("author:").to_string();
                } else if word.starts_with("repo:") {
                    filter.repo = word.trim_start_matches("repo:").to_string();
                } else if word.starts_with("state:") {
                    // State values are case-insensitive and mapped to canonical forms
                    let state_str = word.trim_start_matches("state:").to_ascii_lowercase();
                    filter.state = match state_str.as_str() {
                        "open" => "Open".to_string(),
                        "draft" => "Draft".to_string(),
                        "resolved" | "closed" => "Resolved".to_string(),
                        "canceled" => "Canceled".to_string(),
                        _ => String::new(), // Invalid state values are ignored
                    };
                } else if word.starts_with("title:") {
                    // Explicit title keyword, the value is added to title parts
                    title_parts.push(word.trim_start_matches("title:").to_string());
                } else {
                    // Any other word without a keyword prefix is considered part of the title
                    title_parts.push(word.to_string());
                }
                (filter, title_parts)
            },
        );

        // Join all collected title parts into the final title string
        let final_title = title_parts_accumulator.join(" ");

        Ok(Filter {
            title: final_title,
            author: filter_accumulator.author,
            repo: filter_accumulator.repo,
            state: filter_accumulator.state,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let filters = Filter::parse("title:Rust Programming author:JohnDoe").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "JohnDoe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        // only title can have space
        let filters = Filter::parse("title:Rust Programming author:John Doe").unwrap();
        assert_eq!(filters.title, "Rust Programming Doe");
        assert_eq!(filters.author, "John");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("author:JohnDoe").unwrap();
        assert!(filters.title.is_empty());
        assert_eq!(filters.author, "JohnDoe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("Rust Programming").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters =
            Filter::parse("Rust Programming author:JohnDoe repo:LedgerHQ/ledger-live").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "JohnDoe");
        assert_eq!(filters.repo, "LedgerHQ/ledger-live");
        assert!(filters.state.is_empty());

        let filters = Filter::parse("title:Rust Programming state:open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");

        let filters = Filter::parse("title:Rust Programming state:Open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");

        let filters = Filter::parse("title:Rust Programming state:Resolved").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");

        let filters = Filter::parse("title:Rust Programming state:closed").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");

        let filters = Filter::parse("title:Rust Programming state:Pasfini").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
    }
}
