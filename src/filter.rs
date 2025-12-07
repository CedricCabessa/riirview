use anyhow::Result;

#[derive(Debug)]
pub struct Filter {
    pub title: String,
    pub author: String,
    pub repo: String,
    pub state: String,
    pub type_: String,
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
                    type_: String::new(),
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
                } else if word.starts_with("type:") {
                    let type_str = word.trim_start_matches("type:").to_ascii_lowercase();
                    filter.type_ = match type_str.as_str() {
                        "pr" | "pullrequest" => "PullRequest".to_string(),
                        "issue" | "bug" => "Issue".to_string(),
                        "release" => "Release".to_string(),
                        _ => String::new(), // Invalid type values are ignored
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
            type_: filter_accumulator.type_,
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
        assert!(filters.type_.is_empty());

        // only title can have space
        let filters = Filter::parse("title:Rust Programming author:John Doe").unwrap();
        assert_eq!(filters.title, "Rust Programming Doe");
        assert_eq!(filters.author, "John");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("author:JohnDoe").unwrap();
        assert!(filters.title.is_empty());
        assert_eq!(filters.author, "JohnDoe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("Rust Programming").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
        assert!(filters.type_.is_empty());

        let filters =
            Filter::parse("Rust Programming author:JohnDoe repo:LedgerHQ/ledger-live").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert_eq!(filters.author, "JohnDoe");
        assert_eq!(filters.repo, "LedgerHQ/ledger-live");
        assert!(filters.state.is_empty());
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("title:Rust Programming state:open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("title:Rust Programming state:Open").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Open");
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("title:Rust Programming state:Resolved").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("title:Rust Programming state:closed").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert_eq!(filters.state, "Resolved");
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("title:Rust Programming state:Pasfini").unwrap();
        assert_eq!(filters.title, "Rust Programming");
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());
        assert!(filters.type_.is_empty());

        let filters = Filter::parse("type:pr").unwrap();
        assert_eq!(filters.type_, "PullRequest");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:pullrequest").unwrap();
        assert_eq!(filters.type_, "PullRequest");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:PR").unwrap();
        assert_eq!(filters.type_, "PullRequest");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:PullRequest").unwrap();
        assert_eq!(filters.type_, "PullRequest");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:issue").unwrap();
        assert_eq!(filters.type_, "Issue");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:bug").unwrap();
        assert_eq!(filters.type_, "Issue");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:BUG").unwrap();
        assert_eq!(filters.type_, "Issue");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:Issue").unwrap();
        assert_eq!(filters.type_, "Issue");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:release").unwrap();
        assert_eq!(filters.type_, "Release");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:RELEASE").unwrap();
        assert_eq!(filters.type_, "Release");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:invalid").unwrap();
        assert!(filters.type_.is_empty());
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("title:Rust type:pr author:JohnDoe").unwrap();
        assert_eq!(filters.title, "Rust");
        assert_eq!(filters.type_, "PullRequest");
        assert_eq!(filters.author, "JohnDoe");
        assert!(filters.repo.is_empty());
        assert!(filters.state.is_empty());

        let filters = Filter::parse("type:issue state:open repo:myrepo").unwrap();
        assert_eq!(filters.type_, "Issue");
        assert_eq!(filters.state, "Open");
        assert_eq!(filters.repo, "myrepo");
        assert!(filters.title.is_empty());
        assert!(filters.author.is_empty());
    }
}
