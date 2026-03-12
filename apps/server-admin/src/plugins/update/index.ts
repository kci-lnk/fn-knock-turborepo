import { Elysia } from "elysia";
import { updateManager } from "../../lib/update-manager";

export const updatePlugin = new Elysia({ name: "plugin-update" }).decorate("updateManager", updateManager);
