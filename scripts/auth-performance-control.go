// Build from the sibling Go module, passing this file's absolute path.
// This controls only the explicitly addressed isolated benchmark gateway.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"time"

	"go-reauth-proxy/pkg/grpc/pb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
	"google.golang.org/protobuf/types/known/emptypb"
)

func main() {
	addr := flag.String("addr", "127.0.0.1:27996", "isolated benchmark control address")
	token := flag.String("token", "", "synthetic internal RPC token")
	ttl := flag.Int("cache-ttl", 1, "both positive and negative cache TTLs (0 or 1)")
	readOnly := flag.Bool("read-only", false, "only verify the live configuration, do not flush/change it")
	flag.Parse()
	if *addr != "127.0.0.1:27996" || *token != "isolated-auth-performance-20260923" || (*ttl != 0 && *ttl != 1) {
		fatal(fmt.Errorf("refusing non-benchmark address, token or TTL"))
	}
	conn, err := grpc.NewClient(*addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		fatal(err)
	}
	defer conn.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	defer cancel()
	ctx = metadata.NewOutgoingContext(ctx, metadata.Pairs("x-fn-knock-internal-rpc-token", *token))
	client := pb.NewGatewayControlServiceClient(conn)
	cfg, err := client.GetAuthConfig(ctx, &emptypb.Empty{})
	if err != nil {
		fatal(err)
	}
	if !*readOnly {
		cfg.AuthCacheTtlSeconds = int32(*ttl)
		cfg.AuthCacheUnauthorizedTtlSeconds = int32(*ttl)
		if _, err = client.SetAuthConfig(ctx, cfg); err != nil {
			fatal(err)
		}
	}
	actual, err := client.GetAuthConfig(ctx, &emptypb.Empty{})
	if err != nil {
		fatal(err)
	}
	if actual.AuthCacheTtlSeconds != int32(*ttl) || actual.AuthCacheUnauthorizedTtlSeconds != int32(*ttl) {
		fatal(fmt.Errorf("live cache TTL readback mismatch"))
	}
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"runtime_verified": true, "source": "grpc_GetAuthConfig", "auth_cache_ttl_seconds": actual.AuthCacheTtlSeconds, "auth_cache_unauthorized_ttl_seconds": actual.AuthCacheUnauthorizedTtlSeconds}); err != nil {
		fatal(err)
	}
}

func fatal(err error) { fmt.Fprintln(os.Stderr, err); os.Exit(1) }
