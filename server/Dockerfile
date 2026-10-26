FROM golang:1.27.3-alpine3.24 AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
RUN go mod verify
COPY . .
RUN CGO_ENABLED=0 go build -o relay-server .

FROM alpine:3.24.2
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/relay-server /usr/local/bin/
EXPOSE 8080
HEALTHCHECK CMD wget -q --spider http://localhost:8080/health || exit 1
CMD ["relay-server"]
