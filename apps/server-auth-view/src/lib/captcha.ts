import CryptoJS from 'crypto-js';
import type { CaptchaSubmission } from '@frontend-core/captcha/types';

const POW_HASH_BATCH_SIZE = 2000;
const SUPPORTED_POW_ALGORITHMS = ['SHA-256', 'SHA-384', 'SHA-512'] as const;

type PowAlgorithm = (typeof SUPPORTED_POW_ALGORITHMS)[number];

export type PowChallenge = {
    algorithm: PowAlgorithm;
    challenge: string;
    maxnumber: number;
    salt: string;
    signature: string;
};

const hashPowInput = (algorithm: PowAlgorithm, input: string): string => {
    switch (algorithm) {
        case 'SHA-256':
            return CryptoJS.SHA256(input).toString(CryptoJS.enc.Hex);
        case 'SHA-384':
            return CryptoJS.SHA384(input).toString(CryptoJS.enc.Hex);
        case 'SHA-512':
            return CryptoJS.SHA512(input).toString(CryptoJS.enc.Hex);
        default:
            throw new Error(`Unsupported PoW algorithm: ${algorithm}`);
    }
};

export const normalizePowChallenge = (payload: unknown): PowChallenge => {
    const raw = payload as Partial<PowChallenge> | null;
    const algorithmRaw = String(raw?.algorithm || 'SHA-256').toUpperCase();
    if (!SUPPORTED_POW_ALGORITHMS.includes(algorithmRaw as PowAlgorithm)) {
        throw new Error('不支持的 PoW 算法');
    }

    const challenge = String(raw?.challenge || '').toLowerCase();
    const salt = String(raw?.salt || '');
    const signature = String(raw?.signature || '');
    const maxnumber = Number(raw?.maxnumber);
    if (!challenge || !salt || !signature || !Number.isFinite(maxnumber) || maxnumber < 0) {
        throw new Error('PoW challenge 数据无效');
    }

    return {
        algorithm: algorithmRaw as PowAlgorithm,
        challenge,
        maxnumber: Math.floor(maxnumber),
        salt,
        signature,
    };
};

export const solvePowChallenge = async (challenge: PowChallenge): Promise<number> => {
    for (let number = 0; number <= challenge.maxnumber; number += 1) {
        const digest = hashPowInput(challenge.algorithm, `${challenge.salt}${number}`).toLowerCase();
        if (digest === challenge.challenge) {
            return number;
        }
        if (number > 0 && number % POW_HASH_BATCH_SIZE === 0) {
            await new Promise((resolve) => setTimeout(resolve, 0));
        }
    }

    throw new Error('PoW 求解失败，请刷新页面后重试');
};

export const buildPowSubmission = (challenge: PowChallenge, number: number): CaptchaSubmission => ({
    provider: 'pow',
    proof: btoa(JSON.stringify({
        algorithm: challenge.algorithm,
        challenge: challenge.challenge,
        number,
        salt: challenge.salt,
        signature: challenge.signature,
    })),
});
