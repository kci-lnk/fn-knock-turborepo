import SHA256 from "crypto-js/sha256";
import SHA384 from "crypto-js/sha384";
import SHA512 from "crypto-js/sha512";
import Hex from "crypto-js/enc-hex";

type PowChallenge = {
  algorithm: "SHA-256" | "SHA-384" | "SHA-512";
  challenge: string;
  maxnumber: number;
  salt: string;
};

const hash = (algorithm: PowChallenge["algorithm"], input: string) => {
  switch (algorithm) {
    case "SHA-256":
      return SHA256(input).toString(Hex);
    case "SHA-384":
      return SHA384(input).toString(Hex);
    case "SHA-512":
      return SHA512(input).toString(Hex);
  }
};

self.onmessage = (event: MessageEvent<PowChallenge>) => {
  const challenge = event.data;
  for (let number = 0; number <= challenge.maxnumber; number += 1) {
    if (
      hash(challenge.algorithm, `${challenge.salt}${number}`).toLowerCase() ===
      challenge.challenge
    ) {
      self.postMessage({ number });
      return;
    }
  }
  self.postMessage({ error: "powSolveFailed" });
};
