FROM node:23-alpine

RUN corepack enable pnpm
WORKDIR /app
COPY . .
RUN pnpm install --frozen-lockfile --prod

USER node
EXPOSE 8080
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
ENTRYPOINT [ "pnpm", "run", "start" ]