create table online_users (
    id    integer primary key autoincrement,
    time  timestamp not null,
    count integer
);

create table downloads (
    id   integer primary key autoincrement,
    time timestamp not null,
    ip   text,
    file text
);

create index file_idx on downloads (file);
create index ip_idx on downloads (ip);
