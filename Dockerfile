FROM rust:1.48

WORKDIR /usr/src/myweblog-api

COPY . .

RUN cargo install --path .

CMD ["myweblog-api"]