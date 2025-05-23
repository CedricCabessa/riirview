// @generated automatically by Diesel CLI.

diesel::table! {
    notifications (id) {
        id -> Text,
        title -> Text,
        repo -> Text,
        url -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        state -> Text,
        author -> Text,
        updated_at -> Timestamp,
        unread -> Bool,
        done -> Bool,
        score -> Integer,
        score_boost -> Integer,
        reason -> Text,
    }
}
