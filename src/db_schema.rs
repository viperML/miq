// @generated automatically by Diesel CLI.

diesel::table! {
    paths (id) {
        id -> Nullable<Integer>,
        fs_path -> Text,
    }
}
