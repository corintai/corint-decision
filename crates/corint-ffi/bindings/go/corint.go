package corint

/*
#cgo LDFLAGS: -L../../target/release -lcorint_ffi
#cgo darwin LDFLAGS: -Wl,-rpath,@loader_path/../../target/release
#cgo linux LDFLAGS: -Wl,-rpath,$ORIGIN/../../target/release

#include <stdlib.h>

// Forward declarations of C functions
void* corint_engine_new(const char* repository_path);
void* corint_engine_new_from_database(const char* database_url);
char* corint_engine_decide(void* engine, const char* request_json);
void corint_engine_free(void* engine);
void corint_string_free(char* s);
char* corint_version();
void corint_init_logging();
*/
import "C"
import (
	"encoding/json"
	"errors"
	"unsafe"
)

// DecisionOptions represents request options
type DecisionOptions struct {
	EnableTrace bool `json:"enable_trace"`
}

// DecisionRequest represents a decision request
type DecisionRequest struct {
	EventData map[string]interface{}   `json:"event_data"`
	Features  map[string]interface{}   `json:"features,omitempty"`
	API       map[string]interface{}   `json:"api,omitempty"`
	Service   map[string]interface{}   `json:"service,omitempty"`
	LLM       map[string]interface{}   `json:"llm,omitempty"`
	Vars      map[string]interface{}   `json:"vars,omitempty"`
	Metadata  map[string]string        `json:"metadata,omitempty"`
	Options   DecisionOptions          `json:"options"`
}

// DecisionResponse represents a decision response
type DecisionResponse struct {
	Decision string                 `json:"decision"`
	Actions  []interface{}          `json:"actions"`
	Trace    map[string]interface{} `json:"trace,omitempty"`
	Metadata map[string]interface{} `json:"metadata,omitempty"`
}

// DecisionEngine represents a CORINT decision engine
type DecisionEngine struct {
	handle unsafe.Pointer
}

// NewEngine creates a new decision engine from a file system repository
func NewEngine(repositoryPath string) (*DecisionEngine, error) {
	cPath := C.CString(repositoryPath)
	defer C.free(unsafe.Pointer(cPath))

	handle := C.corint_engine_new(cPath)
	if handle == nil {
		return nil, errors.New("failed to create decision engine")
	}

	return &DecisionEngine{handle: handle}, nil
}

// NewEngineFromDatabase creates a new decision engine from a database
func NewEngineFromDatabase(databaseURL string) (*DecisionEngine, error) {
	cURL := C.CString(databaseURL)
	defer C.free(unsafe.Pointer(cURL))

	handle := C.corint_engine_new_from_database(cURL)
	if handle == nil {
		return nil, errors.New("failed to create decision engine from database")
	}

	return &DecisionEngine{handle: handle}, nil
}

// Decide executes a decision
func (e *DecisionEngine) Decide(request *DecisionRequest) (*DecisionResponse, error) {
	if e.handle == nil {
		return nil, errors.New("engine has been closed")
	}

	// Convert request to JSON
	requestJSON, err := json.Marshal(request)
	if err != nil {
		return nil, err
	}

	cRequest := C.CString(string(requestJSON))
	defer C.free(unsafe.Pointer(cRequest))

	// Call FFI function
	resultPtr := C.corint_engine_decide(e.handle, cRequest)
	if resultPtr == nil {
		return nil, errors.New("decision execution failed")
	}
	defer C.corint_string_free(resultPtr)

	// Convert result to string
	resultJSON := C.GoString(resultPtr)

	// Parse response
	var response DecisionResponse
	if err := json.Unmarshal([]byte(resultJSON), &response); err != nil {
		// Check if it's an error response
		var errorResp struct {
			Error   string `json:"error"`
			Success bool   `json:"success"`
		}
		if json.Unmarshal([]byte(resultJSON), &errorResp) == nil && errorResp.Error != "" {
			return nil, errors.New(errorResp.Error)
		}
		return nil, err
	}

	return &response, nil
}

// Close closes the engine and frees resources
func (e *DecisionEngine) Close() {
	if e.handle != nil {
		C.corint_engine_free(e.handle)
		e.handle = nil
	}
}

// Version returns the CORINT version
func Version() string {
	versionPtr := C.corint_version()
	defer C.corint_string_free(versionPtr)
	return C.GoString(versionPtr)
}

// InitLogging initializes the logging system
func InitLogging() {
	C.corint_init_logging()
}
