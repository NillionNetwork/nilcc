import { z } from "zod";
import { Uuid } from "#/common/types";

export const WorkloadTier = z
  .object({
    tierId: Uuid.openapi({ description: "The identifier for this tier." }),
    name: z.string().openapi({ description: "The name of this tier." }),
    cpus: z
      .number()
      .openapi({ description: "The number of CPUs included in this tier." }),
    gpus: z
      .number()
      .openapi({ description: "The number of GPUs included in this tier." }),
    memoryMb: z.number().openapi({
      description: "The amount of MB of RAM included in this tier.",
    }),
    diskGb: z.number().openapi({
      description: "The amount of GB of disk included in this tier.",
    }),
    cost: z.number().openapi({
      description: "The cost per minute in credits for this tier.",
    }),
  })
  .openapi({ description: "A workload tier." });
export type WorkloadTier = z.infer<typeof WorkloadTier>;

export const CreateWorkloadTierRequest = z
  .object({
    name: z.string().openapi({ description: "The name of the tier." }),
    cpus: z
      .number()
      .openapi({ description: "The number of CPUs included in the tier." }),
    gpus: z
      .number()
      .openapi({ description: "The number of GPUs included in the tier." }),
    memoryMb: z.number().openapi({
      description: "The amount of MB of RAM included in the tier.",
    }),
    diskGb: z.number().openapi({
      description: "The amount of GB of disk included in the tier.",
    }),
    cost: z
      .number()
      .positive()
      .openapi({ description: "The cost per minute in credits for the tier." }),
  })
  .openapi({ description: "A request to create a tier." });
export type CreateWorkloadTierRequest = z.infer<
  typeof CreateWorkloadTierRequest
>;

export const DeleteWorkloadTierRequest = z
  .object({
    tierId: z.string().openapi({ description: "The tier identifier." }),
  })
  .openapi({ description: "A request to delete a tier." });
export type DeleteWorkloadTierRequest = z.infer<
  typeof DeleteWorkloadTierRequest
>;
