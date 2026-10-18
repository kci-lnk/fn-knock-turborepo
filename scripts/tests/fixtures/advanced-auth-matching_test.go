// Evaluate the exact Rust payload accepted by the pinned gateway control service.
package proxy

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"testing"

	"go-reauth-proxy/pkg/models"
)

func TestFnKnockAdvancedAuthRequestMatching(t *testing.T) {
	data, err := os.ReadFile(os.Getenv("FN_KNOCK_TEST_AUTH_PAYLOAD"))
	if err != nil {
		t.Fatal(err)
	}
	var rules []models.HostRule
	if err := json.Unmarshal(data, &rules); err != nil {
		t.Fatal(err)
	}
	if len(rules) != 6 {
		t.Fatalf("got %d rules, want 6", len(rules))
	}
	for _, rule := range rules {
		condition := rule.AdvancedAuth.Groups[0].Conditions[0]
		policy, err := compileAdvancedAuthPolicy(rule.AdvancedAuth)
		if err != nil {
			t.Fatal(err)
		}
		for _, tc := range []struct {
			name, value string
			present     bool
		}{
			{"missing", "", false}, {"empty", "", true},
			{"matching", " Keep Case And Spaces ", true},
			{"wrong-case", " keep case and spaces ", true},
			{"trimmed", "Keep Case And Spaces", true},
		} {
			t.Run(condition.Target+"/"+condition.Operator+"/"+tc.name, func(t *testing.T) {
				request := httptest.NewRequest(http.MethodGet, "https://"+rule.Host+"/private", nil)
				if tc.present {
					if condition.Target == "request_header" {
						// Deliberately bypass Header.Set to test lowercase header names.
						request.Header["x-custom"] = []string{tc.value}
					} else {
						request.URL.RawQuery = url.Values{"x-CuSTom": {tc.value}}.Encode()
					}
				}
				want := tc.present && tc.value == " Keep Case And Spaces "
				if condition.Operator == "exists" {
					want = tc.present
				}
				if condition.Operator == "not_exists" {
					want = !tc.present
				}
				if got := policy.evaluate(request, "192.0.2.1") != nil; got != want {
					t.Fatalf("rule match = %v, want %v", got, want)
				}
			})
		}
	}
}
