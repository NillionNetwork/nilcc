import * as fs from "node:fs";
import type { SchemaObject } from "ajv";

const COMPOSE_SPEC_GIT_HASH = "37cc49e897219b2843f90f296c715339e3c1fae8";
const COMPOSE_SPEC_URL = `https://raw.githubusercontent.com/compose-spec/compose-spec/${COMPOSE_SPEC_GIT_HASH}/schema/compose-spec.json`;

const response = await fetch(COMPOSE_SPEC_URL);
const schema = (await response.json()) as SchemaObject;
schema.$schema = schema.$schema?.replace(/^https/, "http");

const serializedSchema = JSON.stringify(schema, null, 2);

fs.writeFile("src/compose/schema.json", serializedSchema, (err) => {
  if (err) {
    console.error("Error writing to file:", err);
  } else {
    console.log("");
  }
});
