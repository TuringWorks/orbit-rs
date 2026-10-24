# Date and time with fixed inputs only.

SELECT extract(year from date '2026-03-15'), extract(month from date '2026-03-15'), extract(day from date '2026-03-15'), extract(dow from date '2026-03-15'), extract(doy from date '2026-03-15'), extract(quarter from date '2026-03-15'), extract(week from date '2026-03-15')

SELECT extract(hour from timestamp '2026-03-15 10:30:45'), extract(minute from timestamp '2026-03-15 10:30:45'), extract(second from timestamp '2026-03-15 10:30:45'), extract(epoch from timestamp '1970-01-02 00:00:00')

SELECT date_part('year', timestamp '2026-03-15 10:30:45'), date_trunc('month', timestamp '2026-03-15 10:30:45'), date_trunc('day', timestamp '2026-03-15 10:30:45'), date_trunc('hour', timestamp '2026-03-15 10:30:45')

SELECT date '2026-03-15' + 1, date '2026-03-15' - 1, date '2026-03-15' + interval '1 month', date '2026-01-31' + interval '1 month'

SELECT timestamp '2026-03-15 10:30:00' - timestamp '2026-03-14 08:00:00'

SELECT age(timestamp '2026-03-15', timestamp '2024-01-10')

SELECT interval '1 day' * 3, interval '2 hours' + interval '30 minutes', interval '90 minutes', interval '1 year 2 months'

SELECT make_date(2026, 3, 15), make_time(10, 30, 0), make_timestamp(2026, 3, 15, 10, 30, 0)

SELECT to_char(timestamp '2026-03-15 10:30:45', 'YYYY-MM-DD HH24:MI:SS'), to_char(date '2026-03-15', 'Day, DD Mon YYYY'), to_char(date '2026-03-15', 'Month')

SELECT to_date('15/03/2026', 'DD/MM/YYYY'), to_timestamp('2026-03-15 10:30:45', 'YYYY-MM-DD HH24:MI:SS')

SELECT date '2026-03-15' < date '2026-03-16', timestamp '2026-03-15 10:00' = timestamp '2026-03-15 10:00:00', date '2026-03-15' = timestamp '2026-03-15 00:00:00'

SELECT (date '2026-03-15', date '2026-03-20') OVERLAPS (date '2026-03-18', date '2026-03-25')

SELECT generate_series(date '2026-03-01', date '2026-03-04', interval '1 day')

SELECT timestamptz '2026-03-15 10:30:00+00' AT TIME ZONE 'UTC', timestamp '2026-03-15 10:30:00' AT TIME ZONE 'UTC' AT TIME ZONE 'UTC'

SELECT isfinite(date '2026-03-15'), date 'infinity' > date '2026-03-15', '-infinity'::timestamp < '2000-01-01'::timestamp

SELECT justify_hours(interval '36 hours'), justify_days(interval '35 days')
