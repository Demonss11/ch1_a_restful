//! Семантический профиль API.
//!
//! Профиль — это третий агент привязки (после HTTP и медиатипа). Он фиксирует
//! значение каждого имени свойства, `rel` и действия, чтобы M2M-клиенты могли
//! понимать API без внешней документации. Пока профиль не меняется — клиент
//! остаётся работоспособным при изменениях URL, схемы и т.п.

/// Медиатип самого документа профиля.
pub const MEDIA_TYPE: &str = "application/vnd.risk-compiler.v1+profile+json";

/// Документ профиля. Хранится как `&'static str`, чтобы:
/// 1) он был доступен для ревью как обычный JSON;
/// 2) не парсился на каждом запросе;
/// 3) его нельзя было случайно «испортить» при рефакторинге кода.
pub const PROFILE_DOCUMENT: &str = r#"{
  "profile": "risk-compiler.v1",
  "version": "1.0.0",
  "revision": "2026-11-09",
  "description": "Semantic profile for risk-compiler-a. Defines all property names, link relations, actions, and error details. Clients SHOULD bind to this profile together with HTTP and the media type, rather than to specific URLs or object schemas.",
  "mediaType": "application/vnd.risk-compiler.v1+json",
  "entryPoint": "/",
  "profileUrl": "/profile",

  "resources": {
    "program": {
      "description": "A named risk model: the primary unit of work. Owns its build history.",
      "href": "/programs/{slug}",
      "properties": {
        "slug": {
          "type": "string",
          "description": "Stable, URL-safe identifier of the program.",
          "pattern": "^[A-Za-z0-9_-]+$",
          "maxLength": 128,
          "required": true
        },
        "title": {
          "type": "string",
          "description": "Human-readable name of the program.",
          "minLength": 1,
          "maxLength": 200,
          "required": true
        },
        "source": {
          "type": "string",
          "description": "Last source bundle submitted for compilation (opaque string: path, URI, hash). Validated only for non-emptiness.",
          "minLength": 1,
          "required": true
        },
        "current": {
          "type": ["string", "null"],
          "format": "uuid",
          "description": "UUID of the currently active build, if any.",
          "required": false
        },
        "state": {
          "type": ["string", "null"],
          "enum": ["queued", "ready", "failed", null],
          "description": "State of the current build, if any.",
          "required": false
        }
      }
    },
    "build": {
      "description": "A single compilation attempt of a program (evidence layer). Belongs to exactly one program.",
      "href": "/builds/{id}",
      "properties": {
        "id": {
          "type": "string",
          "format": "uuid",
          "description": "Unique build identifier.",
          "required": true
        },
        "program": {
          "type": "string",
          "description": "Slug of the program this build belongs to (hypermedia reference, not a bare UUID).",
          "required": true
        },
        "n": {
          "type": "integer",
          "minimum": 1,
          "description": "Sequential build number within the program (starts at 1, unique per program).",
          "required": true
        },
        "state": {
          "type": "string",
          "enum": ["queued", "ready", "failed"],
          "description": "Current build state.",
          "required": true
        },
        "log": {
          "type": "array",
          "items": {"type": "string"},
          "description": "Append-only human-readable log entries.",
          "required": true
        },
        "artifact": {
          "type": ["string", "null"],
          "description": "Opaque artifact identifier (16-char hex). Present only when state == \"ready\".",
          "required": false
        }
      }
    },
    "ProgramCollection": {
      "description": "List of programs in creation order.",
      "href": "/programs",
      "properties": {
        "total": {"type": "integer", "required": true},
        "items": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["slug", "title", "state", "_links"],
            "properties": {
              "slug": {"type": "string"},
              "title": {"type": "string"},
              "state": {"type": ["string", "null"], "enum": ["queued", "ready", "failed", null]}
            }
          }
        },
        "_links": {"type": "array", "required": true}
      }
    },
    "BuildCollection": {
      "description": "Full audit trail of builds for a program, sorted by n ascending.",
      "href": "/programs/{slug}/builds",
      "properties": {
        "slug": {"type": "string", "required": true},
        "total": {"type": "integer", "required": true},
        "items": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["id", "n", "state", "_links"],
            "properties": {
              "id": {"type": "string", "format": "uuid"},
              "n": {"type": "integer", "minimum": 1},
              "state": {"type": "string", "enum": ["queued", "ready", "failed"]}
            }
          }
        },
        "_links": {"type": "array", "required": true}
      }
    }
  },

  "enums": {
    "buildState": {
      "values": ["queued", "ready", "failed"],
      "transitions": {
        "queued": ["ready", "failed"],
        "ready": [],
        "failed": []
      }
    }
  },

  "linkRelations": {
    "self":          "Canonical URI of the current resource.",
    "entry":         "API entry point (/).",
    "profile":       "This semantic profile document.",
    "programs":      "Collection of all programs.",
    "program":       "Program resource.",
    "builds":        "Collection of builds for a program.",
    "build":         "Build resource (never an action relation).",
    "current-build": "Currently active build of a program (only present when program.current is not null).",
    "create-program": "Action: create a program (POST /programs). Lives on the collection, not the root.",
    "create-build":   "Action: rebuild a program (POST /programs/{slug}/builds)."
  },

  "actions": {
    "list-programs": {
      "method": "GET",
      "href": "/programs",
      "description": "List all programs in creation order.",
      "contentType": null,
      "input": null,
      "responses": {"200": "ProgramCollection"}
    },
    "create-program": {
      "method": "POST",
      "href": "/programs",
      "description": "Create a new program. Atomically creates the program and its first build (state queued); program.current points to it immediately.",
      "contentType": "application/json",
      "input": {
        "type": "object",
        "required": ["slug", "title", "source"],
        "additionalProperties": false,
        "properties": {
          "slug":   {"type": "string", "pattern": "^[A-Za-z0-9_-]+$", "maxLength": 128},
          "title":  {"type": "string", "minLength": 1, "maxLength": 200},
          "source": {"type": "string", "minLength": 1}
        }
      },
      "responses": {
        "201": "Program",
        "400": "BadRequest",
        "409": "Conflict"
      }
    },
    "get-program": {
      "method": "GET",
      "href": "/programs/{slug}",
      "description": "Read a single program.",
      "contentType": null,
      "input": null,
      "responses": {"200": "Program", "404": "NotFound"}
    },
    "list-builds": {
      "method": "GET",
      "href": "/programs/{slug}/builds",
      "description": "List all builds for a program (audit trail), sorted by n ascending.",
      "contentType": null,
      "input": null,
      "responses": {"200": "BuildCollection", "404": "NotFound"}
    },
    "create-build": {
      "method": "POST",
      "href": "/programs/{slug}/builds",
      "description": "Rebuild a program from a new source. Program.current switches to the new build (state queued) immediately.",
      "contentType": "application/json",
      "input": {
        "type": "object",
        "required": ["source"],
        "additionalProperties": false,
        "properties": {
          "source": {"type": "string", "minLength": 1}
        }
      },
      "responses": {"201": "Build", "400": "BadRequest", "404": "NotFound"}
    },
    "get-build": {
      "method": "GET",
      "href": "/builds/{id}",
      "description": "Read a single build.",
      "contentType": null,
      "input": null,
      "responses": {"200": "Build", "404": "NotFound"}
    }
  },

  "errors": {
    "contentType": "application/problem+json",
    "schema": {
      "type": "object",
      "required": ["type", "title", "status", "detail"],
      "properties": {
        "type":   {"type": "string", "format": "uri", "description": "URI reference identifying the error type."},
        "title":  {"type": "string", "description": "Short, human-readable summary."},
        "status": {"type": "integer", "description": "HTTP status code."},
        "detail": {"type": "string", "description": "Human-readable explanation."},
        "_links": {"type": "array", "description": "Related links (e.g. entry point)."}
      }
    },
    "statuses": {
      "400": "Bad Request — malformed or invalid input.",
      "404": "Not Found — unknown resource.",
      "405": "Method Not Allowed — unsupported HTTP method.",
      "409": "Conflict — resource already exists.",
      "500": "Internal Server Error — unexpected error."
    }
  },

  "_links": [
    {"rel": "self",     "method": "GET", "href": "/profile"},
    {"rel": "entry",    "method": "GET", "href": "/"},
    {"rel": "programs", "method": "GET", "href": "/programs"}
  ]
}"#;
