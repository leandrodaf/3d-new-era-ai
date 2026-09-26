-- The paid plan is supported through Buy Me a Coffee now, which pays out
-- through the Stripe account connected to it. Rows left from Paddle stay as
-- they are: `plan_of` never asks which provider paid, so anyone who already
-- pays keeps the plan until the period they paid for runs out.
alter table subscriptions alter column provider set default 'buymeacoffee';

-- When the event that last wrote this row was made, by Buy Me a Coffee's
-- clock. Its deliveries carry no timestamp of their own and are retried, so
-- this is what tells a replay — or a delivery that overtook another — from
-- news: an event older than the one already written leaves the row alone.
alter table subscriptions add column provider_event_at timestamptz;
