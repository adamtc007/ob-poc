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
  [4] = 2,
  [5] = 2,
  [6] = 6,
  [7] = 6,
  [8] = 8,
  [9] = 8,
  [10] = 10,
  [11] = 11,
  [12] = 11,
  [13] = 13,
  [14] = 13,
  [15] = 10,
  [16] = 11,
  [17] = 17,
  [18] = 13,
  [19] = 10,
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
  [44] = 24,
  [45] = 45,
  [46] = 42,
  [47] = 47,
  [48] = 48,
  [49] = 47,
  [50] = 48,
  [51] = 51,
  [52] = 45,
  [53] = 27,
  [54] = 31,
  [55] = 29,
  [56] = 35,
  [57] = 32,
  [58] = 26,
  [59] = 26,
  [60] = 38,
  [61] = 34,
  [62] = 33,
  [63] = 35,
  [64] = 36,
  [65] = 28,
  [66] = 32,
  [67] = 67,
  [68] = 30,
  [69] = 37,
  [70] = 25,
  [71] = 71,
  [72] = 72,
  [73] = 73,
  [74] = 71,
  [75] = 73,
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
      if (lookahead == '@') ADVANCE(51);
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
  [4] = {.lex_state = 0},
  [5] = {.lex_state = 0},
  [6] = {.lex_state = 1},
  [7] = {.lex_state = 1},
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
  [39] = {.lex_state = 1},
  [40] = {.lex_state = 0},
  [41] = {.lex_state = 0},
  [42] = {.lex_state = 13},
  [43] = {.lex_state = 13},
  [44] = {.lex_state = 13},
  [45] = {.lex_state = 2},
  [46] = {.lex_state = 13},
  [47] = {.lex_state = 13},
  [48] = {.lex_state = 2},
  [49] = {.lex_state = 13},
  [50] = {.lex_state = 2},
  [51] = {.lex_state = 2},
  [52] = {.lex_state = 2},
  [53] = {.lex_state = 13},
  [54] = {.lex_state = 13},
  [55] = {.lex_state = 13},
  [56] = {.lex_state = 13},
  [57] = {.lex_state = 13},
  [58] = {.lex_state = 13},
  [59] = {.lex_state = 0},
  [60] = {.lex_state = 13},
  [61] = {.lex_state = 13},
  [62] = {.lex_state = 13},
  [63] = {.lex_state = 0},
  [64] = {.lex_state = 13},
  [65] = {.lex_state = 13},
  [66] = {.lex_state = 0},
  [67] = {.lex_state = 13},
  [68] = {.lex_state = 13},
  [69] = {.lex_state = 13},
  [70] = {.lex_state = 13},
  [71] = {.lex_state = 13},
  [72] = {.lex_state = 0},
  [73] = {.lex_state = 13},
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
    [sym_source_file] = STATE(72),
    [sym__statement] = STATE(40),
    [sym_list] = STATE(40),
    [aux_sym_source_file_repeat1] = STATE(40),
    [ts_builtin_sym_end] = ACTIONS(5),
    [anon_sym_LPAREN] = ACTIONS(7),
    [sym_comment] = ACTIONS(9),
  },
  [2] = {
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
    [anon_sym_RPAREN] = ACTIONS(67),
    [anon_sym_COLONas] = ACTIONS(15),
    [sym_verb_name] = ACTIONS(69),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(71),
    [anon_sym_true] = ACTIONS(25),
    [anon_sym_false] = ACTIONS(25),
    [sym_null_literal] = ACTIONS(73),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [5] = {
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
    [anon_sym_RPAREN] = ACTIONS(75),
    [anon_sym_COLONas] = ACTIONS(15),
    [sym_verb_name] = ACTIONS(77),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(79),
    [anon_sym_true] = ACTIONS(25),
    [anon_sym_false] = ACTIONS(25),
    [sym_null_literal] = ACTIONS(81),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [6] = {
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
    [sym_number] = ACTIONS(83),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(83),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(87),
    [anon_sym_RBRACK] = ACTIONS(89),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [7] = {
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
    [sym_number] = ACTIONS(83),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(83),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(87),
    [anon_sym_RBRACK] = ACTIONS(91),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [8] = {
    [sym__expression] = STATE(6),
    [sym_list] = STATE(6),
    [sym_binding] = STATE(6),
    [sym_keyword] = STATE(6),
    [sym_string] = STATE(6),
    [sym_boolean] = STATE(6),
    [sym_symbol_ref] = STATE(6),
    [sym_array] = STATE(6),
    [sym_map] = STATE(6),
    [aux_sym_array_repeat1] = STATE(6),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(93),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(93),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(87),
    [anon_sym_RBRACK] = ACTIONS(95),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [9] = {
    [sym__expression] = STATE(7),
    [sym_list] = STATE(7),
    [sym_binding] = STATE(7),
    [sym_keyword] = STATE(7),
    [sym_string] = STATE(7),
    [sym_boolean] = STATE(7),
    [sym_symbol_ref] = STATE(7),
    [sym_array] = STATE(7),
    [sym_map] = STATE(7),
    [aux_sym_array_repeat1] = STATE(7),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(97),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(97),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_COMMA] = ACTIONS(87),
    [anon_sym_RBRACK] = ACTIONS(99),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [10] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(101),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [11] = {
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
    [anon_sym_RPAREN] = ACTIONS(101),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(105),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(105),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [12] = {
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
    [anon_sym_RPAREN] = ACTIONS(107),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(109),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(109),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [13] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(111),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [14] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(113),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [15] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(107),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [16] = {
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
    [anon_sym_RPAREN] = ACTIONS(115),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(117),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(117),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [17] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(119),
    [anon_sym_RPAREN] = ACTIONS(122),
    [anon_sym_COLONas] = ACTIONS(124),
    [anon_sym_COLON] = ACTIONS(127),
    [anon_sym_DQUOTE] = ACTIONS(130),
    [sym_number] = ACTIONS(133),
    [anon_sym_true] = ACTIONS(136),
    [anon_sym_false] = ACTIONS(136),
    [sym_null_literal] = ACTIONS(133),
    [anon_sym_AT] = ACTIONS(139),
    [anon_sym_LBRACK] = ACTIONS(142),
    [anon_sym_LBRACE] = ACTIONS(145),
    [sym_comment] = ACTIONS(3),
  },
  [18] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(148),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [19] = {
    [sym__expression] = STATE(17),
    [sym_list] = STATE(17),
    [sym_binding] = STATE(17),
    [sym_keyword] = STATE(17),
    [sym_string] = STATE(17),
    [sym_boolean] = STATE(17),
    [sym_symbol_ref] = STATE(17),
    [sym_array] = STATE(17),
    [sym_map] = STATE(17),
    [aux_sym_list_repeat1] = STATE(17),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_RPAREN] = ACTIONS(115),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(103),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(103),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [20] = {
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
    [sym_number] = ACTIONS(150),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(150),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_RBRACK] = ACTIONS(152),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [21] = {
    [sym__expression] = STATE(8),
    [sym_list] = STATE(8),
    [sym_binding] = STATE(8),
    [sym_keyword] = STATE(8),
    [sym_string] = STATE(8),
    [sym_boolean] = STATE(8),
    [sym_symbol_ref] = STATE(8),
    [sym_array] = STATE(8),
    [sym_map] = STATE(8),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(154),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(154),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_RBRACK] = ACTIONS(156),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
  [22] = {
    [sym__expression] = STATE(67),
    [sym_list] = STATE(67),
    [sym_binding] = STATE(67),
    [sym_keyword] = STATE(67),
    [sym_string] = STATE(67),
    [sym_boolean] = STATE(67),
    [sym_symbol_ref] = STATE(67),
    [sym_array] = STATE(67),
    [sym_map] = STATE(67),
    [anon_sym_LPAREN] = ACTIONS(158),
    [anon_sym_COLONas] = ACTIONS(160),
    [anon_sym_COLON] = ACTIONS(162),
    [anon_sym_DQUOTE] = ACTIONS(164),
    [sym_number] = ACTIONS(166),
    [anon_sym_true] = ACTIONS(168),
    [anon_sym_false] = ACTIONS(168),
    [sym_null_literal] = ACTIONS(166),
    [anon_sym_AT] = ACTIONS(170),
    [anon_sym_LBRACK] = ACTIONS(172),
    [anon_sym_LBRACE] = ACTIONS(174),
    [sym_comment] = ACTIONS(3),
  },
  [23] = {
    [sym__expression] = STATE(39),
    [sym_list] = STATE(39),
    [sym_binding] = STATE(39),
    [sym_keyword] = STATE(39),
    [sym_string] = STATE(39),
    [sym_boolean] = STATE(39),
    [sym_symbol_ref] = STATE(39),
    [sym_array] = STATE(39),
    [sym_map] = STATE(39),
    [anon_sym_LPAREN] = ACTIONS(11),
    [anon_sym_COLONas] = ACTIONS(15),
    [anon_sym_COLON] = ACTIONS(19),
    [anon_sym_DQUOTE] = ACTIONS(21),
    [sym_number] = ACTIONS(176),
    [anon_sym_true] = ACTIONS(85),
    [anon_sym_false] = ACTIONS(85),
    [sym_null_literal] = ACTIONS(176),
    [anon_sym_AT] = ACTIONS(29),
    [anon_sym_LBRACK] = ACTIONS(31),
    [anon_sym_LBRACE] = ACTIONS(33),
    [sym_comment] = ACTIONS(3),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(29), 1,
      anon_sym_AT,
    ACTIONS(180), 1,
      anon_sym_COLON,
    STATE(34), 1,
      sym_symbol_ref,
    ACTIONS(178), 12,
      anon_sym_LPAREN,
      anon_sym_RPAREN,
      anon_sym_COLONas,
      anon_sym_DQUOTE,
      sym_number,
      anon_sym_true,
      anon_sym_false,
      sym_null_literal,
      anon_sym_LBRACK,
      anon_sym_COMMA,
      anon_sym_RBRACK,
      anon_sym_LBRACE,
  [27] = 3,
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
  [49] = 3,
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
  [71] = 3,
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
  [93] = 3,
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
  [115] = 3,
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
  [137] = 3,
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
  [159] = 3,
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
  [181] = 3,
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
  [203] = 3,
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
  [225] = 3,
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
  [247] = 3,
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
  [269] = 3,
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
  [291] = 3,
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
  [313] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(236), 1,
      anon_sym_COLON,
    ACTIONS(234), 13,
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
  [335] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(238), 1,
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
  [356] = 4,
    ACTIONS(7), 1,
      anon_sym_LPAREN,
    ACTIONS(240), 1,
      ts_builtin_sym_end,
    ACTIONS(242), 1,
      sym_comment,
    STATE(41), 3,
      sym__statement,
      sym_list,
      aux_sym_source_file_repeat1,
  [371] = 4,
    ACTIONS(244), 1,
      ts_builtin_sym_end,
    ACTIONS(246), 1,
      anon_sym_LPAREN,
    ACTIONS(249), 1,
      sym_comment,
    STATE(41), 3,
      sym__statement,
      sym_list,
      aux_sym_source_file_repeat1,
  [386] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(252), 1,
      anon_sym_COLON,
    ACTIONS(254), 1,
      anon_sym_RBRACE,
    STATE(22), 1,
      sym_keyword,
    STATE(43), 1,
      aux_sym_map_repeat1,
  [402] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(256), 1,
      anon_sym_COLON,
    ACTIONS(259), 1,
      anon_sym_RBRACE,
    STATE(22), 1,
      sym_keyword,
    STATE(43), 1,
      aux_sym_map_repeat1,
  [418] = 4,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(170), 1,
      anon_sym_AT,
    STATE(61), 1,
      sym_symbol_ref,
    ACTIONS(178), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [432] = 4,
    ACTIONS(261), 1,
      anon_sym_DQUOTE,
    ACTIONS(265), 1,
      sym_comment,
    STATE(50), 1,
      aux_sym_string_repeat1,
    ACTIONS(263), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [446] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(252), 1,
      anon_sym_COLON,
    ACTIONS(267), 1,
      anon_sym_RBRACE,
    STATE(22), 1,
      sym_keyword,
    STATE(43), 1,
      aux_sym_map_repeat1,
  [462] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(252), 1,
      anon_sym_COLON,
    ACTIONS(269), 1,
      anon_sym_RBRACE,
    STATE(22), 1,
      sym_keyword,
    STATE(42), 1,
      aux_sym_map_repeat1,
  [478] = 4,
    ACTIONS(265), 1,
      sym_comment,
    ACTIONS(271), 1,
      anon_sym_DQUOTE,
    STATE(51), 1,
      aux_sym_string_repeat1,
    ACTIONS(273), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [492] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(252), 1,
      anon_sym_COLON,
    ACTIONS(275), 1,
      anon_sym_RBRACE,
    STATE(22), 1,
      sym_keyword,
    STATE(46), 1,
      aux_sym_map_repeat1,
  [508] = 4,
    ACTIONS(265), 1,
      sym_comment,
    ACTIONS(277), 1,
      anon_sym_DQUOTE,
    STATE(51), 1,
      aux_sym_string_repeat1,
    ACTIONS(273), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [522] = 4,
    ACTIONS(265), 1,
      sym_comment,
    ACTIONS(279), 1,
      anon_sym_DQUOTE,
    STATE(51), 1,
      aux_sym_string_repeat1,
    ACTIONS(281), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [536] = 4,
    ACTIONS(265), 1,
      sym_comment,
    ACTIONS(284), 1,
      anon_sym_DQUOTE,
    STATE(48), 1,
      aux_sym_string_repeat1,
    ACTIONS(286), 2,
      aux_sym_string_token1,
      aux_sym_string_token2,
  [550] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(190), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [558] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(206), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [566] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(198), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [574] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(222), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [582] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(210), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [590] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(186), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [598] = 1,
    ACTIONS(186), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [604] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(234), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [612] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(218), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [620] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(214), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [628] = 1,
    ACTIONS(222), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [634] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(226), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [642] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(194), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [650] = 1,
    ACTIONS(210), 3,
      ts_builtin_sym_end,
      anon_sym_LPAREN,
      sym_comment,
  [656] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(259), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [664] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(202), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [672] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(230), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [680] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(182), 2,
      anon_sym_COLON,
      anon_sym_RBRACE,
  [688] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(288), 1,
      aux_sym_keyword_token1,
  [695] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(290), 1,
      ts_builtin_sym_end,
  [702] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(292), 1,
      aux_sym_keyword_token1,
  [709] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(294), 1,
      aux_sym_keyword_token1,
  [716] = 2,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(296), 1,
      aux_sym_keyword_token1,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(24)] = 0,
  [SMALL_STATE(25)] = 27,
  [SMALL_STATE(26)] = 49,
  [SMALL_STATE(27)] = 71,
  [SMALL_STATE(28)] = 93,
  [SMALL_STATE(29)] = 115,
  [SMALL_STATE(30)] = 137,
  [SMALL_STATE(31)] = 159,
  [SMALL_STATE(32)] = 181,
  [SMALL_STATE(33)] = 203,
  [SMALL_STATE(34)] = 225,
  [SMALL_STATE(35)] = 247,
  [SMALL_STATE(36)] = 269,
  [SMALL_STATE(37)] = 291,
  [SMALL_STATE(38)] = 313,
  [SMALL_STATE(39)] = 335,
  [SMALL_STATE(40)] = 356,
  [SMALL_STATE(41)] = 371,
  [SMALL_STATE(42)] = 386,
  [SMALL_STATE(43)] = 402,
  [SMALL_STATE(44)] = 418,
  [SMALL_STATE(45)] = 432,
  [SMALL_STATE(46)] = 446,
  [SMALL_STATE(47)] = 462,
  [SMALL_STATE(48)] = 478,
  [SMALL_STATE(49)] = 492,
  [SMALL_STATE(50)] = 508,
  [SMALL_STATE(51)] = 522,
  [SMALL_STATE(52)] = 536,
  [SMALL_STATE(53)] = 550,
  [SMALL_STATE(54)] = 558,
  [SMALL_STATE(55)] = 566,
  [SMALL_STATE(56)] = 574,
  [SMALL_STATE(57)] = 582,
  [SMALL_STATE(58)] = 590,
  [SMALL_STATE(59)] = 598,
  [SMALL_STATE(60)] = 604,
  [SMALL_STATE(61)] = 612,
  [SMALL_STATE(62)] = 620,
  [SMALL_STATE(63)] = 628,
  [SMALL_STATE(64)] = 634,
  [SMALL_STATE(65)] = 642,
  [SMALL_STATE(66)] = 650,
  [SMALL_STATE(67)] = 656,
  [SMALL_STATE(68)] = 664,
  [SMALL_STATE(69)] = 672,
  [SMALL_STATE(70)] = 680,
  [SMALL_STATE(71)] = 688,
  [SMALL_STATE(72)] = 695,
  [SMALL_STATE(73)] = 702,
  [SMALL_STATE(74)] = 709,
  [SMALL_STATE(75)] = 716,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 0),
  [7] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(24),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(16),
  [19] = {.entry = {.count = 1, .reusable = false}}, SHIFT(73),
  [21] = {.entry = {.count = 1, .reusable = true}}, SHIFT(45),
  [23] = {.entry = {.count = 1, .reusable = true}}, SHIFT(19),
  [25] = {.entry = {.count = 1, .reusable = false}}, SHIFT(38),
  [27] = {.entry = {.count = 1, .reusable = false}}, SHIFT(19),
  [29] = {.entry = {.count = 1, .reusable = true}}, SHIFT(71),
  [31] = {.entry = {.count = 1, .reusable = true}}, SHIFT(20),
  [33] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [35] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(2),
  [38] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(24),
  [41] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(73),
  [44] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(45),
  [47] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(3),
  [50] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(38),
  [53] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(71),
  [56] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(20),
  [59] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(23),
  [62] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2),
  [64] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_array_repeat1, 2), SHIFT_REPEAT(47),
  [67] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [69] = {.entry = {.count = 1, .reusable = true}}, SHIFT(12),
  [71] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [73] = {.entry = {.count = 1, .reusable = false}}, SHIFT(15),
  [75] = {.entry = {.count = 1, .reusable = true}}, SHIFT(56),
  [77] = {.entry = {.count = 1, .reusable = true}}, SHIFT(11),
  [79] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [81] = {.entry = {.count = 1, .reusable = false}}, SHIFT(10),
  [83] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [85] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [87] = {.entry = {.count = 1, .reusable = true}}, SHIFT(23),
  [89] = {.entry = {.count = 1, .reusable = true}}, SHIFT(55),
  [91] = {.entry = {.count = 1, .reusable = true}}, SHIFT(29),
  [93] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [95] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [97] = {.entry = {.count = 1, .reusable = true}}, SHIFT(7),
  [99] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [101] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [103] = {.entry = {.count = 1, .reusable = true}}, SHIFT(17),
  [105] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [107] = {.entry = {.count = 1, .reusable = true}}, SHIFT(66),
  [109] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [111] = {.entry = {.count = 1, .reusable = true}}, SHIFT(58),
  [113] = {.entry = {.count = 1, .reusable = true}}, SHIFT(26),
  [115] = {.entry = {.count = 1, .reusable = true}}, SHIFT(32),
  [117] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [119] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(2),
  [122] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2),
  [124] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(24),
  [127] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(73),
  [130] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(45),
  [133] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(17),
  [136] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(38),
  [139] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(71),
  [142] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(20),
  [145] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_list_repeat1, 2), SHIFT_REPEAT(47),
  [148] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [150] = {.entry = {.count = 1, .reusable = true}}, SHIFT(9),
  [152] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [154] = {.entry = {.count = 1, .reusable = true}}, SHIFT(8),
  [156] = {.entry = {.count = 1, .reusable = true}}, SHIFT(53),
  [158] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [160] = {.entry = {.count = 1, .reusable = true}}, SHIFT(44),
  [162] = {.entry = {.count = 1, .reusable = false}}, SHIFT(75),
  [164] = {.entry = {.count = 1, .reusable = true}}, SHIFT(52),
  [166] = {.entry = {.count = 1, .reusable = true}}, SHIFT(67),
  [168] = {.entry = {.count = 1, .reusable = true}}, SHIFT(60),
  [170] = {.entry = {.count = 1, .reusable = true}}, SHIFT(74),
  [172] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [174] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [176] = {.entry = {.count = 1, .reusable = true}}, SHIFT(39),
  [178] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binding, 1),
  [180] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_binding, 1),
  [182] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_keyword, 2),
  [184] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_keyword, 2),
  [186] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 4),
  [188] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 4),
  [190] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 2),
  [192] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 2),
  [194] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_map, 2),
  [196] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_map, 2),
  [198] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 4),
  [200] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 4),
  [202] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_symbol_ref, 2),
  [204] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_symbol_ref, 2),
  [206] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 3),
  [208] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 3),
  [210] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 3),
  [212] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 3),
  [214] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 3),
  [216] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 3),
  [218] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_binding, 2),
  [220] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_binding, 2),
  [222] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 2),
  [224] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 2),
  [226] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_map, 3),
  [228] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_map, 3),
  [230] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 2),
  [232] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 2),
  [234] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_boolean, 1),
  [236] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_boolean, 1),
  [238] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_array_repeat1, 2),
  [240] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 1),
  [242] = {.entry = {.count = 1, .reusable = true}}, SHIFT(41),
  [244] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2),
  [246] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(4),
  [249] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2), SHIFT_REPEAT(41),
  [252] = {.entry = {.count = 1, .reusable = true}}, SHIFT(73),
  [254] = {.entry = {.count = 1, .reusable = true}}, SHIFT(36),
  [256] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_map_repeat1, 2), SHIFT_REPEAT(73),
  [259] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_map_repeat1, 2),
  [261] = {.entry = {.count = 1, .reusable = false}}, SHIFT(37),
  [263] = {.entry = {.count = 1, .reusable = false}}, SHIFT(50),
  [265] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [267] = {.entry = {.count = 1, .reusable = true}}, SHIFT(64),
  [269] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [271] = {.entry = {.count = 1, .reusable = false}}, SHIFT(54),
  [273] = {.entry = {.count = 1, .reusable = false}}, SHIFT(51),
  [275] = {.entry = {.count = 1, .reusable = true}}, SHIFT(65),
  [277] = {.entry = {.count = 1, .reusable = false}}, SHIFT(31),
  [279] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2),
  [281] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2), SHIFT_REPEAT(51),
  [284] = {.entry = {.count = 1, .reusable = false}}, SHIFT(69),
  [286] = {.entry = {.count = 1, .reusable = false}}, SHIFT(48),
  [288] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [290] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [292] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [294] = {.entry = {.count = 1, .reusable = true}}, SHIFT(68),
  [296] = {.entry = {.count = 1, .reusable = true}}, SHIFT(70),
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
