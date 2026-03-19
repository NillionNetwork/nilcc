FROM node:22-alpine

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY tsconfig.json server.ts ./

EXPOSE 35433

CMD ["npx", "tsx", "server.ts"]
