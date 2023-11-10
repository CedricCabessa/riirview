// @generated automatically by Diesel CLI.

diesel::table! {
    categories (id) {
        id -> Integer,
        uid -> Text,
        name -> Text,
    }
}

diesel::table! {
    prs (id) {
        id -> Integer,
        title -> Text,
        url -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        updated_at -> Text,
        unread -> Bool,
        repo_id -> Integer,
    }
}

diesel::table! {
    repos (id) {
        id -> Integer,
        name -> Text,
        category_id -> Nullable<Integer>,
    }
}

diesel::joinable!(prs -> repos (repo_id));
diesel::joinable!(repos -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(
    categories,
    prs,
    repos,
);
