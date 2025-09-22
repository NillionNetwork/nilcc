import type { Logger } from "pino";
import { z } from "zod";
import { ArtifactVersionDoesNotExist } from "#/common/errors";

export interface ArtifactClient {
  fetchMetadata(version: string): Promise<ArtifactMetadata>;
}

export class DefaultArtifactClient implements ArtifactClient {
  baseUrl: string;
  log: Logger;

  constructor(baseUrl: string, log: Logger) {
    this.baseUrl = baseUrl;
    this.log = log;
  }

  async fetchMetadata(version: string): Promise<ArtifactMetadata> {
    const url = `${this.baseUrl}/${version}/metadata.json`;
    const response = await fetch(url, {
      method: "GET",
    });
    if (!response.ok) {
      if (response.status === 404) {
        throw new ArtifactVersionDoesNotExist();
      }
      const body = await response.text();
      throw new Error(`Failed to fetch version metadata: ${body}`);
    }
    const body = await response.json();
    return ArtifactMetadata.parse(body);
  }
}

export const ArtifactMetadata = z.object({
  built_at: z.number(),
});
export type ArtifactMetadata = z.infer<typeof ArtifactMetadata>;
