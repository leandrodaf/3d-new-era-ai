-- Projects kept in the cloud, and the files made from them.
--
-- A project is the same .newera the desktop writes (a zip with its bundled
-- files), stored whole: small, and backed up with the rest of the database.

create table projects (
    id          text primary key,
    account_id  text not null references accounts (id) on delete cascade,
    name        text not null,
    data        bytea not null,
    size        integer not null,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);
create index projects_account on projects (account_id, updated_at desc);
create unique index projects_name on projects (account_id, lower(name));

-- The project an account's AI works on when no tab is open.
create table active_projects (
    account_id  text primary key references accounts (id) on delete cascade,
    project_id  text not null references projects (id) on delete cascade
);

-- An export (PDF, PNG, GLB, CSV…), served at a secret URL for a day.
create table files (
    token_hash    text primary key,
    account_id    text not null references accounts (id) on delete cascade,
    name          text not null,
    content_type  text not null,
    data          bytea not null,
    expires_at    timestamptz not null
);
