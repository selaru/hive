# ── Build stage ───────────────────────────────────────────────────────────────
FROM node:22-alpine AS builder

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install

COPY . .
RUN npm run build

# ── Runtime: copy dist to a volume, picked up by nginx ───────────────────────
FROM alpine:3.20
WORKDIR /app/dist
COPY --from=builder /app/dist .
# The compose file mounts this as a named volume read by nginx
CMD ["sh", "-c", "cp -r . /dist && echo 'Frontend built' && sleep infinity"]
