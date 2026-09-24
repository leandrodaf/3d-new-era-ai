-- Accounts, plans and the OAuth that lets an AI client act for an account.
--
-- Secrets (session cookies, login links, codes, tokens) are stored only as
-- their SHA-256: a copy of this database opens no account.

create table accounts (
    id          text primary key,
    email       text not null,
    created_at  timestamptz not null default now(),
    deleted_at  timestamptz
);
-- One live account per address, whatever its case.
create unique index accounts_email on accounts (lower(email)) where deleted_at is null;

-- What each plan allows. The tools ask "is there quota left?", never "does
-- this account pay?": a subscription only changes which row applies.
create table plans (
    code    text primary key,
    name    text not null,
    limits  jsonb not null
);
insert into plans (code, name, limits) values
    ('free', 'Free', '{"draft_renders_per_day": 20, "hq_renders_per_month": 0, "projects": 3, "storage_mb": 50}'),
    ('supporter', 'Supporter', '{"draft_renders_per_day": 200, "hq_renders_per_month": 100, "projects": 100, "storage_mb": 2000}');

-- A paid plan, as the payment provider last said. No row: the free plan.
create table subscriptions (
    account_id             text primary key references accounts (id) on delete cascade,
    plan                   text not null references plans (code),
    status                 text not null,
    provider               text not null default 'polar',
    provider_customer      text,
    provider_subscription  text,
    current_period_end     timestamptz,
    updated_at             timestamptz not null default now()
);

-- What an account used, by day and kind, for the quotas.
create table usage (
    account_id  text not null references accounts (id) on delete cascade,
    day         date not null,
    kind        text not null,
    count       integer not null default 0,
    primary key (account_id, day, kind)
);

-- A browser signed in on this site.
create table sessions (
    token_hash  text primary key,
    account_id  text not null references accounts (id) on delete cascade,
    created_at  timestamptz not null default now(),
    expires_at  timestamptz not null
);

-- A sign-in link sent by email; good once, for a few minutes.
create table login_links (
    token_hash  text primary key,
    email       text not null,
    return_to   text not null,
    created_at  timestamptz not null default now(),
    expires_at  timestamptz not null,
    used_at     timestamptz
);

-- AI clients: registered here (dynamic registration) or known by the URL of
-- their metadata document (client ID metadata documents).
create table oauth_clients (
    client_id      text primary key,
    kind           text not null check (kind in ('registered', 'metadata')),
    name           text,
    redirect_uris  text[] not null,
    created_at     timestamptz not null default now(),
    fetched_at     timestamptz
);

-- A code handed to a client on the way back from the consent screen.
create table oauth_codes (
    code_hash       text primary key,
    client_id       text not null,
    account_id      text not null references accounts (id) on delete cascade,
    redirect_uri    text not null,
    code_challenge  text not null,
    scope           text not null,
    resource        text,
    expires_at      timestamptz not null,
    used_at         timestamptz
);

-- Access and refresh tokens. A refresh token is used once and replaced; one
-- used twice revokes its whole family, since one of the two was stolen.
create table oauth_tokens (
    token_hash  text primary key,
    kind        text not null check (kind in ('access', 'refresh')),
    client_id   text not null,
    account_id  text not null references accounts (id) on delete cascade,
    scope       text not null,
    family      text not null,
    expires_at  timestamptz not null,
    revoked_at  timestamptz
);
create index oauth_tokens_family on oauth_tokens (family);
create index oauth_tokens_account on oauth_tokens (account_id);
