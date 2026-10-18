// Copied into the pinned gateway snapshot by test-advanced-auth-gateway-contract.sh.
package admin

import (
	"context"
	"net"
	"os"
	"os/exec"
	"testing"
	"time"

	"go-reauth-proxy/pkg/grpc/pb"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/emptypb"
)

type fnKnockAuthEchoServer struct {
	*GRPCServer
	beforeDisable *pb.HostRules
}

func (s *fnKnockAuthEchoServer) SetHostRules(ctx context.Context, rules *pb.HostRules) (*pb.HostRules, error) {
	for _, rule := range rules.Items {
		if rule.Host == "rollback.example.com" && !rule.GetAdvancedAuth().GetEnabled() && s.beforeDisable == nil {
			var err error
			s.beforeDisable, err = s.GRPCServer.GetHostRules(ctx, &emptypb.Empty{})
			if err != nil {
				return nil, err
			}
		}
	}
	echo, err := s.GRPCServer.SetHostRules(ctx, rules)
	if err == nil {
		for _, rule := range echo.Items {
			if rule.Host == "rollback.example.com" && !rule.GetAdvancedAuth().GetEnabled() {
				rule.AdvancedAuth = &pb.AdvancedAuthConfig{Enabled: true, PolicyVersion: "stale"}
			}
		}
	}
	return echo, err
}

func TestFnKnockAdvancedAuthControlContract(t *testing.T) {
	server := &fnKnockAuthEchoServer{GRPCServer: newGatewayControlTestServer(t, "test-internal-rpc-token")}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	grpcServer := grpc.NewServer()
	pb.RegisterGatewayControlServiceServer(grpcServer, server)
	go func() { _ = grpcServer.Serve(listener) }()
	t.Cleanup(grpcServer.Stop)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	defer cancel()
	command := exec.CommandContext(ctx, "cargo", "test", "--locked", "--lib", "advanced_auth_pinned_gateway_contract", "--", "--ignored", "--nocapture", "--test-threads=1")
	command.Dir = os.Getenv("FN_KNOCK_TEST_RUST_ROOT")
	command.Env = append(os.Environ(), "FN_KNOCK_TEST_GATEWAY_ADDR="+listener.Addr().String())
	output, err := command.CombinedOutput()
	t.Log(string(output))
	if err != nil {
		t.Fatal(err)
	}
	// Stop waits for in-flight handlers before inspecting their test state.
	grpcServer.GracefulStop()
	if server.beforeDisable == nil {
		t.Fatal("rollback fault was not exercised")
	}
	restored := hostRulesBundleToProto(server.admin.ProxyHandler.GetHostRules(), server.admin.ProxyHandler.GetVisibilityPoliciesForHostRules())
	if !proto.Equal(restored, server.beforeDisable) {
		t.Fatal("previous runtime policy was not restored after the rejected acknowledgement")
	}
}
