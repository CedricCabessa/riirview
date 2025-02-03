// @generated automatically by Diesel CLI.

diesel::table! {
    notifications (id) {
        id -> Text,
        title -> Text,
        repo -> Text,
        url -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        updated_at -> Timestamp,
        unread -> Bool,
        done -> Bool,
        score -> Integer,
        pr_state -> Text,
        pr_number -> Integer,
        pr_draft -> Bool,
        pr_merged -> Bool,
        pr_author -> Text,
        score_boost -> Integer,
    }
}
