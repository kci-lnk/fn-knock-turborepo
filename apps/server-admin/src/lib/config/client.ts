import Redis from "ioredis";

const REDIS_CONFIG = {
  host: process.env.REDIS_HOST || "127.0.0.1",
  port: parseInt(process.env.REDIS_PORT || "6379"),
  password: process.env.REDIS_PASSWORD,
};

export const redis = new Redis(REDIS_CONFIG);

redis.on("error", (err) => {
  console.error("Redis connection error:", err);
});
