import { Ajv, type Schema } from "ajv";
import Yaml from "yaml";
import { z } from "zod";
import { InvalidDockerCompose } from "#/common/errors";
import DockerComposeSchema from "./schema.json";

const RESERVED_PORTS: Array<number> = [80, 443];
const RESERVED_CONTAINERS: Array<string> = ["nilcc-attester", "nilcc-proxy"];
const PORT_REGEX =
  /^((?:[\d]+|[\d]+-[\d]+)):(?:[\d]+|[\d]+-[\d]+)(?:\/tcp|\/udp)?$/;

const ContainerName = z
  .string()
  .refine((name) => RESERVED_CONTAINERS.indexOf(name) === -1);

const Port = z.union([
  // A single port.
  z
    .string()
    .transform(Number)
    .pipe(z.number())
    .refine(
      (port) => RESERVED_PORTS.indexOf(port) === -1,
      (port) => ({ message: `port ${port} is reserved` }),
    ),

  // A port range `<left>-<right>`.
  z
    .string()
    .transform((spec) => spec.split("-").map(Number))
    .refine((range) => range.length === 2)
    .pipe(z.tuple([z.number(), z.number()]))
    // Don't allow ranges where lower bound is greater than upper bound
    .refine(
      ([left, right]) => left <= right,
      ([left, right]) => ({ message: `invalid port range ${left}-${right}` }),
    )
    // Make sure the port range doesn't cover a reserved port.
    .refine(
      ([left, right]) =>
        RESERVED_PORTS.every((port) => port < left || port > right),
      ([left, right]) => ({
        message: `port range ${left}-${right} includes reserved ports`,
      }),
    ),
]);
const DockerComposePolicy = z.object({
  services: z.record(
    ContainerName,
    z.object({
      container_name: ContainerName.optional(),
      ports: z
        .union([
          // Handle`<host-ports>:<guest-ports>[/<protocol>]` port notation
          z
            .string()
            .transform((spec) => PORT_REGEX.exec(spec)?.at(1))
            .refine((spec) => spec !== undefined)
            .pipe(Port),

          // Handle the long port definition. We only care about the `published` key here.
          z.object({ published: Port.optional() }),
          // .refine((spec) => spec.published === undefined || Port.parse(spec.published)),

          // Fallback to simply a port, which is always fine.
          z
            .string()
            .transform(Number)
            .pipe(z.number().positive()),
        ])
        .array()
        .optional(),
    }),
  ),
});

export class DockerComposeValidator {
  validate(rawCompose: string, exposedService: string): void {
    const parsedData = this.parseYaml(rawCompose);
    const ajv = new Ajv({ strict: false });
    const validate = ajv.compile(DockerComposeSchema as Schema);
    if (!validate(parsedData)) {
      throw new InvalidDockerCompose(
        `malformed docker compose: ${validate.errors}`,
      );
    }
    const result = DockerComposePolicy.safeParse(parsedData);
    if (!result.success) {
      const message = result.error.issues.at(0)?.message;
      throw new InvalidDockerCompose(`compose validation failed: ${message}`);
    }
    const service = Object.entries(result.data.services).filter(
      ([service, definition]) =>
        service === exposedService ||
        definition.container_name === exposedService,
    );
    if (!service.length) {
      throw new InvalidDockerCompose(
        `exposed service '${exposedService}' not part of docker compose`,
      );
    }
  }

  private parseYaml(rawCompose: string): unknown {
    try {
      return Yaml.parse(rawCompose);
    } catch (e) {
      throw new InvalidDockerCompose(`malformed docker compose yaml: ${e}`);
    }
  }
}
