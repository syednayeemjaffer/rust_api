// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Int8,
        #[max_length = 255]
        profile -> Varchar,
        #[max_length = 255]
        email -> Varchar,
        #[max_length = 50]
        firstname -> Varchar,
        #[max_length = 50]
        lastname -> Varchar,
        #[max_length = 20]
        ph -> Varchar,
        #[max_length = 255]
        password -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}
