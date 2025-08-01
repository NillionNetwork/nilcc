import {
  ChangeResourceRecordSetsCommand,
  CreateHostedZoneCommand,
  ListHostedZonesCommand,
  ListResourceRecordSetsCommand,
  Route53Client,
  type RRType,
} from "@aws-sdk/client-route-53";
import type { Logger } from "pino";

export interface DnsService {
  createRecord(domain: string, to: string, recordType: RRType): Promise<void>;
  deleteRecord(domain: string, recordType: RRType): Promise<void>;
}

export class Route53DnsService implements DnsService {
  protected readonly route53: Route53Client;
  readonly zone: string;
  readonly zoneId: string;
  readonly subdomain: string;
  readonly log: Logger;

  protected constructor(
    zone: string,
    subdomain: string,
    zoneId: string,
    route53: Route53Client,
    log: Logger,
  ) {
    this.zone = zone;
    this.subdomain = subdomain;
    this.route53 = route53;
    this.zoneId = zoneId;
    this.log = log;
  }

  static async create(
    zone: string,
    subdomain: string,
    log: Logger,
  ): Promise<Route53DnsService> {
    const route53 = new Route53Client();
    if (!subdomain.endsWith(zone)) {
      throw new Error(`${subdomain} isn't a subdomain of ${zone}`);
    }

    const zoneId = await Route53DnsService.findHostedZone(route53, zone, log);
    return new Route53DnsService(zone, subdomain, zoneId, route53, log);
  }

  async createRecord(
    domain: string,
    to: string,
    recordType: RRType,
  ): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: this.zoneId,
      ChangeBatch: {
        Changes: [
          {
            Action: "CREATE",
            ResourceRecordSet: {
              Name: fullDomain,
              Type: recordType,
              TTL: 300,
              ResourceRecords: [{ Value: to }],
            },
          },
        ],
      },
    });
    this.log.info(
      `Creating ${recordType} record ${fullDomain} pointing to ${to}`,
    );
    await this.route53.send(command);
  }

  async deleteRecord(domain: string, recordType: RRType): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    this.log.info(`Looking up domain ${fullDomain}`);
    const domainData = await this.findDomain(
      this.zoneId,
      fullDomain,
      recordType,
    );
    if (!domainData) {
      this.log.warn(`Domain ${fullDomain} does not exist, ignoring`);
      return;
    }
    this.log.info(`Removing record for domain ${fullDomain}`);
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: this.zoneId,
      ChangeBatch: {
        Changes: [
          {
            Action: "DELETE",
            ResourceRecordSet: domainData,
          },
        ],
      },
    });
    await this.route53.send(command);
  }

  private static async findHostedZone(
    route53: Route53Client,
    subdomain: string,
    log: Logger,
  ): Promise<string> {
    let marker: string | undefined;
    while (true) {
      log.info(
        `Trying to find hosted zone for domain ${subdomain} using marker ${marker}`,
      );
      const command: ListHostedZonesCommand = new ListHostedZonesCommand({
        Marker: marker,
      });
      const response = await route53.send(command);
      if (!response.HostedZones) {
        throw Error(`Hosted zone not found: ${subdomain}`);
      }

      const hostedZone = response.HostedZones.find(
        (z) => z.Name === `${subdomain}.`,
      );
      if (hostedZone?.Id) {
        return hostedZone.Id;
      }
      if (response.NextMarker === undefined) {
        throw Error(`Hosted zone not found: ${subdomain}`);
      }
      marker = response.NextMarker;
    }
  }

  private async findDomain(zoneId: string, domain: string, recordType: RRType) {
    const command = new ListResourceRecordSetsCommand({
      HostedZoneId: zoneId,
      StartRecordName: `${domain}.`,
      StartRecordType: recordType,
      MaxItems: 1,
    });
    const response = await this.route53.send(command);
    const [recordset, ..._rest] = response.ResourceRecordSets || [null, null];
    return recordset;
  }
}

export class LocalStackDnsService extends Route53DnsService {
  static override async create(
    zone: string,
    subdomain: string,
    log: Logger,
  ): Promise<Route53DnsService> {
    const route53 = new Route53Client({
      endpoint: process.env.APP_LOCALSTACK_URI,
      region: "us-east-1", // LocalStack default region
      credentials: {
        accessKeyId: "test",
        secretAccessKey: "test",
      },
    });

    const zoneId = await LocalStackDnsService.createHostedZone(
      subdomain,
      route53,
    );
    return new LocalStackDnsService(zone, subdomain, zoneId, route53, log);
  }

  private static async createHostedZone(
    subdomain: string,
    route53: Route53Client,
  ): Promise<string> {
    const callerReference = Math.random().toString();

    const createHostedZoneParams = {
      Name: subdomain,
      CallerReference: callerReference,
    };

    const createHostedZoneCommand = new CreateHostedZoneCommand(
      createHostedZoneParams,
    );

    const response = await route53.send(createHostedZoneCommand);
    if (!response.HostedZone || !response.HostedZone.Id) {
      throw new Error(`Failed to create hosted zone: ${subdomain}`);
    }
    return response.HostedZone.Id;
  }
}
