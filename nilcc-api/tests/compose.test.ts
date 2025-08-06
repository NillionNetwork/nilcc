import { describe, expect, test } from "vitest";
import { InvalidDockerCompose } from "#/common/errors";
import { DockerComposeValidator } from "#/compose/validator";

describe("Docker compose", () => {
  test("invalid yaml", () => {
    const rawCompose = `
foo:
  **
`;
    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      InvalidDockerCompose,
    );
  });

  test("invalid compose", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports: "42"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      InvalidDockerCompose,
    );
  });

  test("reserved eab key id env", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    environment:
      FOO: $CADDY_ACME_EAB_KEY_ID
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      InvalidDockerCompose,
    );
  });

  test("reserved eab mac key env", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    environment:
      FOO: $CADDY_ACME_EAB_MAC_KEY
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      InvalidDockerCompose,
    );
  });

  test("valid minimal", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
`;

    const validator = new DockerComposeValidator();
    validator.validate(rawCompose, "api");
  });

  test("valid container name", () => {
    const rawCompose = `
services:
  foo:
    container_name: api
    image: caddy:2
`;

    const validator = new DockerComposeValidator();
    validator.validate(rawCompose, "api");
  });

  test("valid complex", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports:
      - "80"
      - "81:80"
      - "82:80/tcp"
      - "83:80/tcp"
      - "84-85:80/tcp"
      - "86-87:80-81/tcp"
      - "86-87:80-81/udp"
      - published: "88"
        target: 1024
      - published: "89-90"
      - target: 2048
    environment:
      FOO: \${ FOO_VAR }
    command: "caddy"
`;

    const validator = new DockerComposeValidator();
    validator.validate(rawCompose, "api");
  });

  test("reserved 80 port", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports:
      - "80:80"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      new InvalidDockerCompose(
        "compose validation failed: port 80 is reserved",
      ),
    );
  });

  test("reserved 443 port", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports:
      - "443:80"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      new InvalidDockerCompose(
        "compose validation failed: port 443 is reserved",
      ),
    );
  });

  test("reserved port range port", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports:
      - "80-80:80"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      new InvalidDockerCompose(
        "compose validation failed: port range 80-80 includes reserved ports",
      ),
    );
  });

  test("reserved long range", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    ports:
      - target: "80"
        published: "80"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "api")).toThrow(
      new InvalidDockerCompose(
        "compose validation failed: port 80 is reserved",
      ),
    );
  });

  test("container not found", () => {
    const rawCompose = `
services:
  api:
    image: caddy:2
    command: "caddy"
`;

    const validator = new DockerComposeValidator();
    expect(() => validator.validate(rawCompose, "other")).toThrow(
      new InvalidDockerCompose(
        "exposed service 'other' not part of docker compose",
      ),
    );
  });
});
