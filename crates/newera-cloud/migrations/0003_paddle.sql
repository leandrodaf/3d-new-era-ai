-- Paid plans are sold through Paddle now; Polar never went live.
alter table subscriptions alter column provider set default 'paddle';
