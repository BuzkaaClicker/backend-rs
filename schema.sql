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
