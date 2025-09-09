import type { WorkloadTier } from "./workload-tier.dto";
import type { WorkloadTierEntity } from "./workload-tier.entity";

export const workloadTierMapper = {
  entityToResponse(tier: WorkloadTierEntity): WorkloadTier {
    return {
      tierId: tier.id,
      name: tier.name,
      cpus: tier.cpus,
      gpus: tier.gpus,
      memoryMb: tier.memory,
      diskGb: tier.disk,
      cost: tier.cost,
    };
  },
};
