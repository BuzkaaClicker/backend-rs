CREATE DATABASE buzkaaclicker;
\connect buzkaaclicker;

create table online_users
(
    id    serial primary key,
    time  timestamp not null,
    count integer
);

create table downloads
(
    id   serial primary key,
    time timestamp not null,
    ip   inet,
    file varchar(255)
);

CREATE INDEX file_idx ON downloads (file);
CREATE INDEX ip_idx ON downloads (ip);
