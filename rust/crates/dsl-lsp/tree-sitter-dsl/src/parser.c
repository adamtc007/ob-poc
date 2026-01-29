#include <tree_sitter/parser.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 14
#define STATE_COUNT 76
#define LARGE_STATE_COUNT 24
#define SYMBOL_COUNT 37
#define ALIAS_COUNT 0
#define TOKEN_COUNT 21
#define EXTERNAL_TOKEN_COUNT 0
#define FIELD_COUNT 0
#define MAX_ALIAS_SEQUENCE_LENGTH 4
#define PRODUCTION_ID_COUNT 1

enum {
  anon_sym_LPAREN = 1,
  anon_sym_RPAREN = 2,
  anon_sym_COLONas = 3,
  sym_verb_name = 4,
  anon_sym_COLON = 5,
  aux_sym_keyword_token1 = 6,
  anon_sym_DQUOTE = 7,
  aux_sym_string_token1 = 8,
  aux_sym_string_token2 = 9,
  sym_number = 10,
  anon_sym_true = 11,
  anon_sym_false = 12,
  sym_null_literal = 13,
  anon_sym_AT = 14,
  anon_sym_LBRACK = 15,
  anon_sym_COMMA = 16,
  anon_sym_RBRACK = 17,
  anon_sym_LBRACE = 18,
  anon_sym_RBRACE = 19,
  sym_comment = 20,
  sym_source_file = 21,
  sym__statement = 22,
  sym__expression = 23,
  sym_list = 24,
  sym_binding = 25,
  sym_keyword = 26,
  sym_string = 27,
  sym_boolean = 28,
  sym_symbol_ref = 29,
  sym_array = 30,
  sym_map = 31,
  aux_sym_source_file_repeat1 = 32,
  aux_sym_list_repeat1 = 33,
  aux_sym_string_repeat1 = 34,
  aux_sym_array_repeat1 = 35,
  aux_sym_map_repeat1 = 36,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [anon_sym_LPAREN] = "(",
  [anon_sym_RPAREN] = ")",
  [anon_sym_COLONas] = ":as",
  [sym_verb_name] = "verb_name",
  [anon_sym_COLON] = ":",
  [aux_sym_keyword_token1] = "keyword_token1",
  [anon_sym_DQUOTE] = "\"",
  [aux_sym_string_token1] = "string_token1",
  [aux_sym_string_token2] = "string_token2",
  [sym_number] = "number",
  [anon_sym_true] = "true",
  [anon_sym_false] = "false",
  [sym_null_literal] = "null_literal",
  [anon_sym_AT] = "@",
  [anon_sym_LBRACK] = "[",
  [anon_sym_COMMA] = ",",
  [anon_sym_RBRACK] = "]",
  [anon_sym_LBRACE] = "{",
  [anon_sym_RBRACE] = "}",
  [sym_comment] = "comment",
  [sym_source_file] = "source_file",
  [sym__statement] = "_statement",
  [sym__expression] = "_expression",
  [sym_list] = "list",
  [sym_binding] = "binding",
  [sym_keyword] = "keyword",
  [sym_string] = "string",
  [sym_boolean] = "boolean",
  [sym_symbol_ref] = "symbol_ref",
  [sym_array] = "array",
  [sym_map] = "map",
  [aux_sym_source_file_repeat1] = "source_file_repeat1",
  [aux_sym_list_repeat1] = "list_repeat1",
  [aux_sym_string_repeat1] = "string_repeat1",
  [aux_sym_array_repeat1] = "array_repeat1",
  [aux_sym_map_repeat1] = "map_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [anon_sym_LPAREN] = anon_sym_LPAREN,
  [anon_sym_RPAREN] = anon_sym_RPAREN,
  [anon_sym_COLONas] = anon_sym_COLONas,
  [sym_verb_name] = sym_verb_name,
  [anon_sym_COLON] = anon_sym_COLON,
  [aux_sym_keyword_token1] = aux_sym_keyword_token1,
  [anon_sym_DQUOTE] = anon_sym_DQUOTE,
  [aux_sym_string_token1] = aux_sym_string_token1,
  [aux_sym_string_token2] = aux_sym_string_token2,
  [sym_number] = sym_number,
  [anon_sym_true] = anon_sym_true,
  [anon_sym_false] = anon_sym_false,
  [sym_null_literal] = sym_null_literal,
  [anon_sym_AT] = anon_sym_AT,
  [anon_sym_LBRACK] = anon_sym_LBRACK,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [anon_sym_RBRACK] = anon_sym_RBRACK,
  [anon_sym_LBRACE] = anon_sym_LBRACE,
  [anon_sym_RBRACE] = anon_sym_RBRACE,
  [sym_comment] = sym_comment,
  [sym_source_file] = sym_source_file,
  [sym__statement] = sym__statement,
  [sym__expression] = sym__expression,
  [sym_list] = sym_list,
  [sym_binding] = sym_binding,
  [sym_keyword] = sym_keyword,
  [sym_string] = sym_string,
  [sym_boolean] = sym_boolean,
  [sym_symbol_ref] = sym_symbol_ref,
  [sym_array] = sym_array,
  [sym_map] = sym_map,
  [aux_sym_source_file_repeat1] = aux_sym_source_file_repeat1,
  [aux_sym_list_repeat1] = aux_sym_list_repeat1,
  [aux_sym_string_repeat1] = aux_sym_string_repeat1,
  [aux_sym_array_repeat1] = aux_sym_array_repeat1,
  [aux_sym_map_repeat1] = aux_sym_map_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [anon_sym_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COLONas] = {
    .visible = true,
    .named = false,
  },
  [sym_verb_name] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_COLON] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_keyword_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_DQUOTE] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_string_token1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_string_token2] = {
    .visible = false,
    .named = false,
  },
  [sym_number] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_true] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_false] = {
    .visible = true,
    .named = false,
  },
  [sym_null_literal] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_AT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACE] = {
    .visible = true,
    .named = false,
  },
  [sym_comment] = {
    .visible = true,
    .named = true,
  },
  [sym_source_file] = {
    .visible = true,
    .named = true,
  },
  [sym__statement] = {
    .visible = false,
    .named = true,
  },
  [sym__expression] = {
    .visible = false,
    .named = true,
  },
  [sym_list] = {
    .visible = true,
    .named = true,
  },
  [sym_binding] = {
    .visible = true,
    .named = true,
  },
  [sym_keyword] = {
    .visible = true,
    .named = true,
  },
  [sym_string] = {
    .visible = true,
    .named = true,
  },
  [sym_boolean] = {
    .visible = true,
    .named = true,
  },
  [sym_symbol_ref] = {
    .visible = true,
    .named = true,
  },
  [sym_array] = {
    .visible = true,
    .named = true,
  },
  [sym_map] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_source_file_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_list_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_string_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_array_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_map_repeat1] = {
    .visible = false,
    .named = false,
  },
};

static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {
  [0] = {0},
};

static const uint16_t ts_non_terminal_alias_map[] = {
  0,
};

static const TSStateId ts_primary_state_ids[STATE_COUNT] = {
  [0] = 0,
  [1] = 1,
  [2] = 2,
  [3] = 3,
  [4] = 4,
  [5] = 5,
  [6] = 2,
  [7] = 2,
  [8] = 4,
  [9] = 5,
  [10] = 10,
  [11] = 11,
  [12] = 10,
  [13] = 13,
  [14] = 13,
  [15] = 13,
  [16] = 11,
  [17] = 11,
  [18] = 10,
  [19] = 19,
  [20] = 20,
  [21] = 20,
  [22] = 22,
  [23] = 23,
  [24] = 24,
  [25] = 25,
  [26] = 26,
  [27] = 27,
  [28] = 28,
  [29] = 29,
  [30] = 30,
  [31] = 31,
  [32] = 32,
  [33] = 33,
  [34] = 34,
  [35] = 35,
  [36] = 36,
  [37] = 37,
  [38] = 38,
  [39] = 39,
  [40] = 40,
  [41] = 41,
  [42] = 42,
  [43] = 43,
  [44] = 41,
  [45] = 45,
  [46] = 46,
  [47] = 47,
  [48] = 47,
  [49] = 43,
  [50] = 46,
  [51] = 24,
  [52] = 29,
  [53] = 36,
  [54] = 37,
  [55] = 31,
  [56] = 29,
  [57] = 27,
  [58] = 58,
  [59] = 59,
  [60] = 30,
  [61] = 28,
  [62] = 27,
  [63] = 31,
  [64] = 33,
  [65] = 25,
  [66] = 59,
  [67] = 32,
  [68] = 34,
  [69] = 26,
  [70] = 35,
  [71] = 71,
  [72] = 72,
  [73] = 73,
  [74] = 71,
  [75] = 72,
};

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(29);
      if (lookahead == '"') ADVANCE(37);
      if (lookahead == '(') ADVANCE(30);
      if (lookahead == ')') ADVANCE(31);
      if (lookahead == ',') ADVANCE(53);
      if (lookahead == '-') ADVANCE(25);
      if (lookahead == ':') ADVANCE(35);
      if (lookahead == ';') ADVANCE(14);
      if (lookahead == '@') ADVANCE(51);
      if (lookahead == '[') ADVANCE(52);
      if (lookahead == '\\') ADVANCE(28);
      if (lookahead == ']') ADVANCE(54);
      if (lookahead == 'f') ADVANCE(3);
      if (lookahead == 'n') ADVANCE(6);
      if (lookahead == 't') ADVANCE(9);
      if (lookahead == '{') ADVANCE(55);
      if (lookahead == '}') ADVANCE(56);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(0)
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(43);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 1:
      if (lookahead == '"') ADVANCE(37);
      if (lookahead == '(') ADVANCE(30);
      if (lookahead == ')') ADVANCE(31);
      if (lookahead == ',') ADVANCE(53);
      if (lookahead == '-') ADVANCE(25);
      if (lookahead == ':') ADVANCE(35);
      if (lookahead == ';') ADVANCE(14);
      if (lookahead == '@') ADVANCE(51);
      if (lookahead == '[') ADVANCE(52);
      if (lookahead == ']') ADVANCE(54);
      if (lookahead == 'f') ADVANCE(15);
      if (lookahead == 'n') ADVANCE(18);
      if (lookahead == 't') ADVANCE(21);
      if (lookahead == '{') ADVANCE(55);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(1)
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(43);
      END_STATE();
    case 2:
      if (lookahead == '"') ADVANCE(37);
      if (lookahead == ';') ADVANCE(40);
      if (lookahead == '\\') ADVANCE(28);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') ADVANCE(39);
      if (lookahead != 0) ADVANCE(41);
      END_STATE();
    case 3:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'a') ADVANCE(7);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('b' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 4:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'e') ADVANCE(46);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 5:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'e') ADVANCE(48);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 6:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'i') ADVANCE(8);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 7:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'l') ADVANCE(10);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 8:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'l') ADVANCE(50);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 9:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'r') ADVANCE(11);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 10:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 's') ADVANCE(5);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 11:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == 'u') ADVANCE(4);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 12:
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 13:
      if (lookahead == ':') ADVANCE(34);
      if (lookahead == ';') ADVANCE(14);
      if (lookahead == '}') ADVANCE(56);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(13)
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(36);
      END_STATE();
    case 14:
      if (lookahead == ';') ADVANCE(57);
      END_STATE();
    case 15:
      if (lookahead == 'a') ADVANCE(20);
      END_STATE();
    case 16:
      if (lookahead == 'e') ADVANCE(45);
      END_STATE();
    case 17:
      if (lookahead == 'e') ADVANCE(47);
      END_STATE();
    case 18:
      if (lookahead == 'i') ADVANCE(19);
      END_STATE();
    case 19:
      if (lookahead == 'l') ADVANCE(49);
      END_STATE();
    case 20:
      if (lookahead == 'l') ADVANCE(23);
      END_STATE();
    case 21:
      if (lookahead == 'r') ADVANCE(24);
      END_STATE();
    case 22:
      if (lookahead == 's') ADVANCE(32);
      END_STATE();
    case 23:
      if (lookahead == 's') ADVANCE(17);
      END_STATE();
    case 24:
      if (lookahead == 'u') ADVANCE(16);
      END_STATE();
    case 25:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(43);
      END_STATE();
    case 26:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(44);
      END_STATE();
    case 27:
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(33);
      END_STATE();
    case 28:
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(42);
      END_STATE();
    case 29:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 30:
      ACCEPT_TOKEN(anon_sym_LPAREN);
      END_STATE();
    case 31:
      ACCEPT_TOKEN(anon_sym_RPAREN);
      END_STATE();
    case 32:
      ACCEPT_TOKEN(anon_sym_COLONas);
      END_STATE();
    case 33:
      ACCEPT_TOKEN(sym_verb_name);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(33);
      END_STATE();
    case 34:
      ACCEPT_TOKEN(anon_sym_COLON);
      END_STATE();
    case 35:
      ACCEPT_TOKEN(anon_sym_COLON);
      if (lookahead == 'a') ADVANCE(22);
      END_STATE();
    case 36:
      ACCEPT_TOKEN(aux_sym_keyword_token1);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(36);
      END_STATE();
    case 37:
      ACCEPT_TOKEN(anon_sym_DQUOTE);
      END_STATE();
    case 38:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '\n') ADVANCE(41);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(57);
      if (lookahead != 0) ADVANCE(38);
      END_STATE();
    case 39:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == ';') ADVANCE(40);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') ADVANCE(39);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(41);
      END_STATE();
    case 40:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == ';') ADVANCE(38);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(41);
      END_STATE();
    case 41:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(41);
      END_STATE();
    case 42:
      ACCEPT_TOKEN(aux_sym_string_token2);
      END_STATE();
    case 43:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(26);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(43);
      END_STATE();
    case 44:
      ACCEPT_TOKEN(sym_number);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(44);
      END_STATE();
    case 45:
      ACCEPT_TOKEN(anon_sym_true);
      END_STATE();
    case 46:
      ACCEPT_TOKEN(anon_sym_true);
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 47:
      ACCEPT_TOKEN(anon_sym_false);
      END_STATE();
    case 48:
      ACCEPT_TOKEN(anon_sym_false);
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 49:
      ACCEPT_TOKEN(sym_null_literal);
      END_STATE();
    case 50:
      ACCEPT_TOKEN(sym_null_literal);
      if (lookahead == '.') ADVANCE(27);
      if (lookahead == '-' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(12);
      END_STATE();
    case 51:
      ACCEPT_TOKEN(anon_sym_AT);
      END_STATE();
    case 52:
      ACCEPT_TOKEN(anon_sym_LBRACK);
      END_STATE();
    case 53:
      ACCEPT_TOKEN(anon_sym_COMMA);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(anon_sym_RBRACK);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(anon_sym_LBRACE);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(anon_sym_RBRACE);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(57);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0},
  [1] = {.lex_state = 0},
  [2] = {.lex_state = 0},
  [3] = {.lex_state = 1},
  [4] = {.lex_state = 1},
  [5] = {.lex_state = 1},
  [6] = {.lex_state = 0},
  [7] = {.lex_state = 0},
  [8] = {.lex_state = 1},
  [9] = {.lex_state = 1},
  [10] = {.lex_state = 1},
  [11] = {.lex_state = 1},
  [12] = {.lex_state = 1},
  [13] = {.lex_state = 1},
  [14] = {.lex_state = 1},
  [15] = {.lex_state = 1},
  [16] = {.lex_state = 1},
  [17] = {.lex_state = 1},
  [18] = {.lex_state = 1},
  [19] = {.lex_state = 1},
  [20] = {.lex_state = 1},
  [21] = {.lex_state = 1},
  [22] = {.lex_state = 1},
  [23] = {.lex_state = 1},
  [24] = {.lex_state = 1},
  [25] = {.lex_state = 1},
  [26] = {.lex_state = 1},
  [27] = {.lex_state = 1},
  [28] = {.lex_state = 1},
  [29] = {.lex_state = 1},
  [30] = {.lex_state = 1},
  [31] = {.lex_state = 1},
  [32] = {.lex_state = 1},
  [33] = {.lex_state = 1},
  [34] = {.lex_state = 1},
  [35] = {.lex_state = 1},
  [36] = {.lex_state = 1},
  [37] = {.lex_state = 1},
  [38] = {.lex_state = 1},
  [39] = {.lex_state = 0},
  [40] = {.lex_state = 0},
  [41] = {.lex_state = 13},
  [42] = {.lex_state = 13},
  [43] = {.lex_state = 2},
  [44] = {.lex_state = 13},
  [45] = {.lex_state = 2},
  [46] = {.lex_state = 2},
  [47] = {.lex_state = 13},
  [48] = {.lex_state = 13},
  [49] = {.lex_state = 2},
  [50] = {.lex_state = 2},
  [51] = {.lex_state = 13},
  [52] = {.lex_state = 0},
  [53] = {.lex_state = 13},
  [54] = {.lex_state = 13},
  [55] = {.lex_state = 13},
  [56] = {.lex_state = 13},
  [57] = {.lex_state = 13},
  [58] = {.lex_state = 13},
  [59] = {.lex_state = 0},
  [60] = {.lex_state = 13},
  [61] = {.lex_state = 13},
  [62] = {.lex_state = 0},
  [63] = {.lex_state = 0},
  [64] = {.lex_state = 13},
  [65] = {.lex_state = 13},
  [66] = {.lex_state = 0},
  [67] = {.lex_state = 13},
  [68] = {.lex_state = 13},
  [69] = {.lex_state = 13},
  [70] = {.lex_state = 13},
  [71] = {.lex_state = 13},
  [72] = {.lex_state = 13},
  [73] = {.lex_state = 0},
  [74] = {.lex_state = 13},
  [75] = {.lex_state = 13},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [ts_builtin_sym_end] = ACTIONS(1),
    [anon_sym_LPAREN] = ACTIONS(1),
    [anon_sym_RPAREN] = ACTIONS(1),
    [anon_sym_COLONas] = ACTIONS(1),
    [sym_verb_name] = ACTIONS(1),
    [anon_sym_COLON] = ACTIONS(1),
    [anon_sym_DQUOTE] = ACTIONS(1),
    [aux_sym_string_token2] = ACTIONS(1),
    [sym_number] = ACTIONS(1),
    [anon_sym_true] = ACTIONS(1),
    [anon_sym_false] = ACTIONS(1),
    [sym_null_literal] = ACTIONS(1),
    [anon_sym_AT] = ACTIONS(1),
    [anon_sym_LBRACK] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [anon_sym_RBRACK] = ACTIONS(1),
    [anon_sym_LBRACE] = ACTIONS(1),
    [anon_sym_RBRACE] = ACTIONS(1),
    [sym_comment] = ACTIONS(3),
  },
  [1] = {
    [sym_source_file] = STATE(73),
    [sym__statement] = STATE(39),
    [sym_list] = STATE(39),
    [aux_sym_source_file_repeat1] = STATE(39),
    [ts_builtin_sym_end] = ACTIONS(5),
    [anon_sym_LPAREN] = ACTIONS(7),
    [sym_comment] = ACTIONS(9),
  },
  [2] = {
    [sym__expression] = STATE(14),
    [sym_list] = STATE(14),
    [sym_binding] = STATE(14),
    [sym_keyword] = STATE(14),
    [sym_string] = STATE(14),
    [sym_boolean] = STATE(14),
    [sym_symbol_ref] = STATE(14),
    [sym_array] = STATE(14),
    [sym_map] = STATE(14),
    [aux_sym_list_repeat1] = STATE(14),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(13),
    [anon_sym_COLONas] = ACTIONS(15),
    [sym_verb_name] = ACTIONS(17),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(23),
    [anon_sym_true] = ACTIONS(25),
    [anon_sym_false] = ACTIONS(25),
    [sym_null_literal] = ACTIONS(27),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [3] = {
    [sym__expression] = STATE(3),
    [sym_list] = STATE(3),
    [sym_binding] = STATE(3),
    [sym_keyword] = STATE(3),
    [sym_string] = STATE(3),
    [sym_boolean] = STATE(3),
    [sym_symbol_ref] = STATE(3),
    [sym_array] = STATE(3),
    [sym_map] = STATE(3),
    [aux_sym_array_repeat1] = STATE(3),
    [anon_sym_LPAREN] = ACTIONS(35),
    [anon_sym_COLONas] = ACTIONS(38),
    [anon_sym_COLON] = ACTIONS(41),
    [anon_sym_DQUOTE] = ACTIONS(44),
    [sym_number] = ACTIONS(47),
    [anon_sym_true] = ACTIONS(50),
    [anon_sym_false] = ACTIONS(50),
    [sym_null_literal] = ACTIONS(47),
    [anon_sym_AT] = ACTIONS(53),
    [anon_sym_LBRACK] = ACTIONS(56),
    [anon_sym_COMMA] = ACTIONS(59),
    [anon_sym_RBRACK] = ACTIONS(62),
    [anon_sym_LBRACE] = ACTIONS(64),
    [sym_comment] = ACTIONS(3),
  },
  [4] = {
    [sym__expression] = STATE(3),
    [sym_list] = STATE(3),
    [sym_binding] = STATE(3),
    [sym_keyword] = STATE(3),
    [sym_string] = STATE(3),
    [sym_boolean] = STATE(3),
    [sym_symbol_ref] = STATE(3),
    [sym_array] = STATE(3),
    [sym_map] = STATE(3),
    [aux_sym_array_repeat1] = STATE(3),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(67),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(67),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(71),
    [anon_sym_RBRACK] = ACTIONS(73),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [5] = {
    [sym__expression] = STATE(4),
    [sym_list] = STATE(4),
    [sym_binding] = STATE(4),
    [sym_keyword] = STATE(4),
    [sym_string] = STATE(4),
    [sym_boolean] = STATE(4),
    [sym_symbol_ref] = STATE(4),
    [sym_array] = STATE(4),
    [sym_map] = STATE(4),
    [aux_sym_array_repeat1] = STATE(4),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(75),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(75),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(71),
    [anon_sym_RBRACK] = ACTIONS(77),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [6] = {
    [sym__expression] = STATE(15),
    [sym_list] = STATE(15),
    [sym_binding] = STATE(15),
    [sym_keyword] = STATE(15),
    [sym_string] = STATE(15),
    [sym_boolean] = STATE(15),
    [sym_symbol_ref] = STATE(15),
    [sym_array] = STATE(15),
    [sym_map] = STATE(15),
    [aux_sym_list_repeat1] = STATE(15),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(79),
    [anon_sym_COLONas] = ACTIONS(15),
    [sym_verb_name] = ACTIONS(81),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(83),
    [anon_sym_true] = ACTIONS(25),
    [anon_sym_false] = ACTIONS(25),
    [sym_null_literal] = ACTIONS(85),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [7] = {
    [sym__expression] = STATE(13),
    [sym_list] = STATE(13),
    [sym_binding] = STATE(13),
    [sym_keyword] = STATE(13),
    [sym_string] = STATE(13),
    [sym_boolean] = STATE(13),
    [sym_symbol_ref] = STATE(13),
    [sym_array] = STATE(13),
    [sym_map] = STATE(13),
    [aux_sym_list_repeat1] = STATE(13),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(87),
    [anon_sym_COLONas] = ACTIONS(15),
    [sym_verb_name] = ACTIONS(89),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(91),
    [anon_sym_true] = ACTIONS(25),
    [anon_sym_false] = ACTIONS(25),
    [sym_null_literal] = ACTIONS(93),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [8] = {
    [sym__expression] = STATE(3),
    [sym_list] = STATE(3),
    [sym_binding] = STATE(3),
    [sym_keyword] = STATE(3),
    [sym_string] = STATE(3),
    [sym_boolean] = STATE(3),
    [sym_symbol_ref] = STATE(3),
    [sym_array] = STATE(3),
    [sym_map] = STATE(3),
    [aux_sym_array_repeat1] = STATE(3),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(67),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(67),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(71),
    [anon_sym_RBRACK] = ACTIONS(95),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [9] = {
    [sym__expression] = STATE(8),
    [sym_list] = STATE(8),
    [sym_binding] = STATE(8),
    [sym_keyword] = STATE(8),
    [sym_string] = STATE(8),
    [sym_boolean] = STATE(8),
    [sym_symbol_ref] = STATE(8),
    [sym_array] = STATE(8),
    [sym_map] = STATE(8),
    [aux_sym_array_repeat1] = STATE(8),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(97),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(97),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(71),
    [anon_sym_RBRACK] = ACTIONS(99),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [10] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(101),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [11] = {
    [sym__expression] = STATE(10),
    [sym_list] = STATE(10),
    [sym_binding] = STATE(10),
    [sym_keyword] = STATE(10),
    [sym_string] = STATE(10),
    [sym_boolean] = STATE(10),
    [sym_symbol_ref] = STATE(10),
    [sym_array] = STATE(10),
    [sym_map] = STATE(10),
    [aux_sym_list_repeat1] = STATE(10),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(105),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(107),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(107),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [12] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(109),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [13] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(111),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [14] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(105),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [15] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(113),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [16] = {
    [sym__expression] = STATE(12),
    [sym_list] = STATE(12),
    [sym_binding] = STATE(12),
    [sym_keyword] = STATE(12),
    [sym_string] = STATE(12),
    [sym_boolean] = STATE(12),
    [sym_symbol_ref] = STATE(12),
    [sym_array] = STATE(12),
    [sym_map] = STATE(12),
    [aux_sym_list_repeat1] = STATE(12),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(111),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(115),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(115),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [17] = {
    [sym__expression] = STATE(18),
    [sym_list] = STATE(18),
    [sym_binding] = STATE(18),
    [sym_keyword] = STATE(18),
    [sym_string] = STATE(18),
    [sym_boolean] = STATE(18),
    [sym_symbol_ref] = STATE(18),
    [sym_array] = STATE(18),
    [sym_map] = STATE(18),
    [aux_sym_list_repeat1] = STATE(18),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(113),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(117),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(117),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [18] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(119),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [19] = {
    [sym__expression] = STATE(19),
    [sym_list] = STATE(19),
    [sym_binding] = STATE(19),
    [sym_keyword] = STATE(19),
    [sym_string] = STATE(19),
    [sym_boolean] = STATE(19),
    [sym_symbol_ref] = STATE(19),
    [sym_array] = STATE(19),
    [sym_map] = STATE(19),
    [aux_sym_list_repeat1] = STATE(19),
    [anon_sym_LPAREN] = ACTIONS(121),
    [anon_sym_RPAREN] = ACTIONS(124),
    [anon_sym_COLONas] = ACTIONS(126),
    [anon_sym_COLON] = ACTIONS(129),
    [anon_sym_DQUOTE] = ACTIONS(132),
    [sym_number] = ACTIONS(135),
    [anon_sym_true] = ACTIONS(138),
    [anon_sym_false] = ACTIONS(138),
    [sym_null_literal] = ACTIONS(135),
    [anon_sym_AT] = ACTIONS(141),
    [anon_sym_LBRACK] = ACTIONS(144),
    [anon_sym_LBRACE] = ACTIONS(147),
    [sym_comment] = ACTIONS(3),
  },
  [20] = {
    [sym__expression] = STATE(5),
    [sym_list] = STATE(5),
    [sym_binding] = STATE(5),
    [sym_keyword] = STATE(5),
    [sym_string] = STATE(5),
    [sym_boolean] = STATE(5),
    [sym_symbol_ref] = STATE(5),
    [sym_array] = STATE(5),
    [sym_map] = STATE(5),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(150),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(150),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_RBRACK] = ACTIONS(152),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [21] = {
    [sym__expression] = STATE(9),
    [sym_list] = STATE(9),
    [sym_binding] = STATE(9),
    [sym_keyword] = STATE(9),
    [sym_string] = STATE(9),
    [sym_boolean] = STATE(9),
    [sym_symbol_ref] = STATE(9),
    [sym_array] = STATE(9),
    [sym_map] = STATE(9),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(154),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(154),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_RBRACK] = ACTIONS(156),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [22] = {
    [sym__expression] = STATE(38),
    [sym_list] = STATE(38),
    [sym_binding] = STATE(38),
    [sym_keyword] = STATE(38),
    [sym_string] = STATE(38),
    [sym_boolean] = STATE(38),
    [sym_symbol_ref] = STATE(38),
    [sym_array] = STATE(38),
    [sym_map] = STATE(38),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(158),
    [anon_sym_true] = ACTIONS(69),
    [anon_sym_false] = ACTIONS(69),
    [sym_null_literal] = ACTIONS(158),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [23] = {
    [sym__expression] = STATE(58),
    [sym_list] = STATE(58),
    [sym_binding] = STATE(58),
    [sym_keyword] = STATE(58),
    [sym_string] = STATE(58),
    [sym_boolean] = STATE(58),
    [sym_symbol_ref] = STATE(58),
    [sym_array] = STATE(58),
    [sym_map] = STATE(58),
    [anon_sym_LPAREN] = ACTIONS(160),
    [anon_sym_COLONas] = ACTIONS(162),
    [anon_sym_COLON] = ACTIONS(164),
    [anon_sym_DQUOTE] = ACTIONS(166),
    [sym_number] = ACTIONS(168),
    [anon_sym_true] = ACTIONS(170),
    [anon_sym_false] = ACTIONS(170),
    [sym_null_literal] = ACTIONS(168),
    [anon_sym_AT] = ACTIONS(172),
    [anon_sym_LBRACK] = ACTIONS(174),
    [anon_sym_LBRACE] = ACTIONS(176),
    [sym_comment] = ACTIONS(3),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(180), 1,
      anon_sym_COLON,
    ACTIONS(178), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [22] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(184), 1,
      anon_sym_COLON,
    ACTIONS(182), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [44] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(188), 1,
      anon_sym_COLON,
    ACTIONS(186), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [66] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(192), 1,
      anon_sym_COLON,
    ACTIONS(190), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [88] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(196), 1,
      anon_sym_COLON,
    ACTIONS(194), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [110] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(200), 1,
      anon_sym_COLON,
    ACTIONS(198), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [132] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(204), 1,
      anon_sym_COLON,
    ACTIONS(202), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [154] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(208), 1,
      anon_sym_COLON,
    ACTIONS(206), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [176] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(212), 1,
      anon_sym_COLON,
    ACTIONS(210), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [198] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(216), 1,
      anon_sym_COLON,
    ACTIONS(214), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [220] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(220), 1,
      anon_sym_COLON,
    ACTIONS(218), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [242] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(224), 1,
      anon_sym_COLON,
    ACTIONS(222), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [264] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(228), 1,
      anon_sym_COLON,
    ACTIONS(226), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [286] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(232), 1,
      anon_sym_COLON,
    ACTIONS(230), 13,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [308] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(234), 1,
      anon_sym_COLON,
    ACTIONS(62), 12,
      anon_sym_LPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_AT,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [329] = 4,
    ACTIONS(7), 1,
      anon_sym_LPAREN,
    ACTIONS(236), 1,
      ts_builtin_sym_end,
    ACTIONS(238), 1,
      sym_comment,
    STATE(40), 3,
      sym__statement,
      sym_list,
      aux_sym_source_file_repeat1,
  [344] = 4,
    ACTIONS(240), 1,
      ts_builtin_sym_end,
    ACTIONS(242), 1,
      anon_sym_LPAREN,
    ACTIONS(245), 1,
      sym_comment,
    STATE(40), 3,
      sym__statement,
      sym_list,
      aux_sym_source_file_repeat1,
  [359] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(248), 1,
      anon_sym_COLON,
    ACTIONS(250), 1,
      anon_sym_RBRACE,
    STATE(23), 1,
      sym_keyword,
    STATE(42), 1,
      aux_sym_map_repeat1,
  [375] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(252), 1,
      anon_sym_COLON,
    ACTIONS(255), 1,
      anon_sym_RBRACE,
    STATE(23), 1,
      sym_keyword,
    STATE(42), 1,
      aux_sym_map_repeat1,
  [391] = 4,
    ACTIONS(257), 1,
      anon_sym_DQUOTE,
    ACTIONS(261), 1,
      sym_comment,
    STATE(50), 1,
      aux_sym_string_repeat1,
    ACTIONS(259), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [405] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(248), 1,
      anon_sym_COLON,
    ACTIONS(263), 1,
      anon_sym_RBRACE,
    STATE(23), 1,
      sym_keyword,
    STATE(42), 1,
      aux_sym_map_repeat1,
  [421] = 4,
    ACTIONS(261), 1,
      sym_comment,
    ACTIONS(265), 1,
      anon_sym_DQUOTE,
    STATE(45), 1,
      aux_sym_string_repeat1,
    ACTIONS(267), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [435] = 4,
    ACTIONS(261), 1,
      sym_comment,
    ACTIONS(270), 1,
      anon_sym_DQUOTE,
    STATE(45), 1,
      aux_sym_string_repeat1,
    ACTIONS(272), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [449] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(248), 1,
      anon_sym_COLON,
    ACTIONS(274), 1,
      anon_sym_RBRACE,
    STATE(23), 1,
      sym_keyword,
    STATE(41), 1,
      aux_sym_map_repeat1,
  [465] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(248), 1,
      anon_sym_COLON,
    ACTIONS(276), 1,
      anon_sym_RBRACE,
    STATE(23), 1,
      sym_keyword,
    STATE(44), 1,
      aux_sym_map_repeat1,
  [481] = 4,
    ACTIONS(261), 1,
      sym_comment,
    ACTIONS(278), 1,
      anon_sym_DQUOTE,
    STATE(46), 1,
      aux_sym_string_repeat1,
    ACTIONS(280), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [495] = 4,
    ACTIONS(261), 1,
      sym_comment,
    ACTIONS(282), 1,
      anon_sym_DQUOTE,
    STATE(45), 1,
      aux_sym_string_repeat1,
    ACTIONS(272), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [509] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(178), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [517] = 1,
    ACTIONS(198), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [523] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(226), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [531] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(230), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [539] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(206), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [547] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(198), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [555] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(190), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [563] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(255), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [571] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(172), 1,
      anon_sym_AT,
    STATE(69), 1,
      sym_symbol_ref,
  [581] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(202), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [589] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(194), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [597] = 1,
    ACTIONS(190), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [603] = 1,
    ACTIONS(206), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [609] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(214), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [617] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(182), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [625] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(29), 1,
      anon_sym_AT,
    STATE(26), 1,
      sym_symbol_ref,
  [635] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(210), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [643] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(218), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [651] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(186), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [659] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(222), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [667] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(284), 1,
      aux_sym_keyword_token1,
  [674] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(286), 1,
      aux_sym_keyword_token1,
  [681] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(288), 1,
      ts_builtin_sym_end,
  [688] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(290), 1,
      aux_sym_keyword_token1,
  [695] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(292), 1,
      aux_sym_keyword_token1,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(24)] = 0,
  [SMALL_STATE(25)] = 22,
  [SMALL_STATE(26)] = 44,
  [SMALL_STATE(27)] = 66,
  [SMALL_STATE(28)] = 88,
  [SMALL_STATE(29)] = 110,
  [SMALL_STATE(30)] = 132,
  [SMALL_STATE(31)] = 154,
  [SMALL_STATE(32)] = 176,
  [SMALL_STATE(33)] = 198,
  [SMALL_STATE(34)] = 220,
  [SMALL_STATE(35)] = 242,
  [SMALL_STATE(36)] = 264,
  [SMALL_STATE(37)] = 286,
  [SMALL_STATE(38)] = 308,
  [SMALL_STATE(39)] = 329,
  [SMALL_STATE(40)] = 344,
  [SMALL_STATE(41)] = 359,
  [SMALL_STATE(42)] = 375,
  [SMALL_STATE(43)] = 391,
  [SMALL_STATE(44)] = 405,
  [SMALL_STATE(45)] = 421,
  [SMALL_STATE(46)] = 435,
  [SMALL_STATE(47)] = 449,
  [SMALL_STATE(48)] = 465,
  [SMALL_STATE(49)] = 481,
  [SMALL_STATE(50)] = 495,
  [SMALL_STATE(51)] = 509,
  [SMALL_STATE(52)] = 517,
  [SMALL_STATE(53)] = 523,
  [SMALL_STATE(54)] = 531,
  [SMALL_STATE(55)] = 539,
  [SMALL_STATE(56)] = 547,
  [SMALL_STATE(57)] = 555,
  [SMALL_STATE(58)] = 563,
  [SMALL_STATE(59)] = 571,
  [SMALL_STATE(60)] = 581,
  [SMALL_STATE(61)] = 589,
  [SMALL_STATE(62)] = 597,
  [SMALL_STATE(63)] = 603,
  [SMALL_STATE(64)] = 609,
  [SMALL_STATE(65)] = 617,
  [SMALL_STATE(66)] = 625,
  [SMALL_STATE(67)] = 635,
  [SMALL_STATE(68)] = 643,
  [SMALL_STATE(69)] = 651,
  [SMALL_STATE(70)] = 659,
  [SMALL_STATE(71)] = 667,
  [SMALL_STATE(72)] = 674,
  [SMALL_STATE(73)] = 681,
  [SMALL_STATE(74)] = 688,
  [SMALL_STATE(75)] = 695,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 0),
  [7] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(39),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(7),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(66),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(11),
  [19] = {.entry = {.count = 1, .reusable = false}}, SHIFT(72),
  [21] = {.entry = {.count = 1, .reusable = true}}, SHIFT(43),
  [23] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [25] = {.entry = {.count = 1, .reusable = false}}, SHIFT(28),
  [27] = {.entry = {.count = 1, .reusable = false}}, SHIFT(14),
  [29] = {.entry = {.count = 1, .reusable = true}}, SHIFT(71),
  [31] = {.entry = {.count = 1, .reusable = true}}, SHIFT(20),
  [33] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [35] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(7),
  [38] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(66),
  [41] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(72),
  [44] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(43),
  [47] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(3),
  [50] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(28),
  [53] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(71),
  [56] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(20),
  [59] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(22),
  [62] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2),
  [64] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(47),
  [67] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [69] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [71] = {.entry = {.count = 1, .reusable = true}}, SHIFT(22),
  [73] = {.entry = {.count = 1, .reusable = true}}, SHIFT(37),
  [75] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [77] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [79] = {.entry = {.count = 1, .reusable = true}}, SHIFT(55),
  [81] = {.entry = {.count = 1, .reusable = true}}, SHIFT(17),
  [83] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [85] = {.entry = {.count = 1, .reusable = false}}, SHIFT(15),
  [87] = {.entry = {.count = 1, .reusable = true}}, SHIFT(31),
  [89] = {.entry = {.count = 1, .reusable = true}}, SHIFT(16),
  [91] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [93] = {.entry = {.count = 1, .reusable = false}}, SHIFT(13),
  [95] = {.entry = {.count = 1, .reusable = true}}, SHIFT(54),
  [97] = {.entry = {.count = 1, .reusable = true}}, SHIFT(8),
  [99] = {.entry = {.count = 1, .reusable = true}}, SHIFT(64),
  [101] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [103] = {.entry = {.count = 1, .reusable = true}}, SHIFT(19),
  [105] = {.entry = {.count = 1, .reusable = true}}, SHIFT(52),
  [107] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [109] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [111] = {.entry = {.count = 1, .reusable = true}}, SHIFT(29),
  [113] = {.entry = {.count = 1, .reusable = true}}, SHIFT(56),
  [115] = {.entry = {.count = 1, .reusable = true}}, SHIFT(12),
  [117] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [119] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [121] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(7),
  [124] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2),
  [126] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(66),
  [129] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(72),
  [132] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(43),
  [135] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(19),
  [138] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(28),
  [141] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(71),
  [144] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(20),
  [147] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(47),
  [150] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [152] = {.entry = {.count = 1, .reusable = true}}, SHIFT(24),
  [154] = {.entry = {.count = 1, .reusable = true}}, SHIFT(9),
  [156] = {.entry = {.count = 1, .reusable = true}}, SHIFT(51),
  [158] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [160] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [162] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [164] = {.entry = {.count = 1, .reusable = false}}, SHIFT(75),
  [166] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [168] = {.entry = {.count = 1, .reusable = true}}, SHIFT(58),
  [170] = {.entry = {.count = 1, .reusable = true}}, SHIFT(61),
  [172] = {.entry = {.count = 1, .reusable = true}}, SHIFT(74),
  [174] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [176] = {.entry = {.count = 1, .reusable = true}}, SHIFT(48),
  [178] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 2),
  [180] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 2),
  [182] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_map, 2),
  [184] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_map, 2),
  [186] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binding, 2),
  [188] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_binding, 2),
  [190] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 4),
  [192] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 4),
  [194] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_boolean, 1),
  [196] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_boolean, 1),
  [198] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 3),
  [200] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 3),
  [202] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 3),
  [204] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 3),
  [206] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 2),
  [208] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 2),
  [210] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_symbol_ref, 2),
  [212] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_symbol_ref, 2),
  [214] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 3),
  [216] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 3),
  [218] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 2),
  [220] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 2),
  [222] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_keyword, 2),
  [224] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_keyword, 2),
  [226] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_map, 3),
  [228] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_map, 3),
  [230] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 4),
  [232] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 4),
  [234] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_array_repeat1, 2),
  [236] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 1),
  [238] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [240] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2),
  [242] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(2),
  [245] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(40),
  [248] = {.entry = {.count = 1, .reusable = true}}, SHIFT(72),
  [250] = {.entry = {.count = 1, .reusable = true}}, SHIFT(36),
  [252] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_map_repeat1, 2), SHIFT_REPEAT(72),
  [255] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_map_repeat1, 2),
  [257] = {.entry = {.count = 1, .reusable = false}}, SHIFT(34),
  [259] = {.entry = {.count = 1, .reusable = false}}, SHIFT(50),
  [261] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [263] = {.entry = {.count = 1, .reusable = true}}, SHIFT(53),
  [265] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2),
  [267] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2), SHIFT_REPEAT(45),
  [270] = {.entry = {.count = 1, .reusable = false}}, SHIFT(60),
  [272] = {.entry = {.count = 1, .reusable = false}}, SHIFT(45),
  [274] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [276] = {.entry = {.count = 1, .reusable = true}}, SHIFT(65),
  [278] = {.entry = {.count = 1, .reusable = false}}, SHIFT(68),
  [280] = {.entry = {.count = 1, .reusable = false}}, SHIFT(46),
  [282] = {.entry = {.count = 1, .reusable = false}}, SHIFT(30),
  [284] = {.entry = {.count = 1, .reusable = true}}, SHIFT(32),
  [286] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [288] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [290] = {.entry = {.count = 1, .reusable = true}}, SHIFT(67),
  [292] = {.entry = {.count = 1, .reusable = true}}, SHIFT(70),
};

#ifdef __cplusplus
extern "C" {
#endif
#ifdef _WIN32
#define extern __declspec(dllexport)
#endif

extern const TSLanguage *tree_sitter_dsl(void) {
  static const TSLanguage language = {
    .version = LANGUAGE_VERSION,
    .symbol_count = SYMBOL_COUNT,
    .alias_count = ALIAS_COUNT,
    .token_count = TOKEN_COUNT,
    .external_token_count = EXTERNAL_TOKEN_COUNT,
    .state_count = STATE_COUNT,
    .large_state_count = LARGE_STATE_COUNT,
    .production_id_count = PRODUCTION_ID_COUNT,
    .field_count = FIELD_COUNT,
    .max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,
    .parse_table = &ts_parse_table[0][0],
    .small_parse_table = ts_small_parse_table,
    .small_parse_table_map = ts_small_parse_table_map,
    .parse_actions = ts_parse_actions,
    .symbol_names = ts_symbol_names,
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
    .primary_state_ids = ts_primary_state_ids,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
