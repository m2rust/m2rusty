#!/usr/bin/env perl
# Generates the wire layer of crates/m2-net from the C++ sources:
#
#   crates/m2-net/src/wire/generated.rs    packet structs, codecs, header
#                                          constants, header map, layout tables
#   tools/differential/net/layouts.inc     X-macro lists for NetDump.cpp
#
# Input: client/include/Client/UI/Packet.h (+ the few structs of GameType.h
# that packets embed) and the CMainPacketHeaderMap in
# client/src/ui/PythonNetworkStream.cpp.  Run from the repository root:
#
#   perl tools/differential/net/gen_packets.pl
#
# Preprocessor: Packet.h is read as the Linux build sees it (GAIDEN,
# __AUCTION__ and USE_AHNLAB_HACKSHIELD undefined).
use strict;
use warnings;

my $packet_h = 'client/include/Client/UI/Packet.h';
my $stream_cpp = 'client/src/ui/PythonNetworkStream.cpp';
my $out_rs = 'crates/m2-net/src/wire/generated.rs';
my $out_inc = 'tools/differential/net/layouts.inc';

sub slurp { local $/; open my $f, '<', $_[0] or die "$_[0]: $!"; return <$f>; }

# ---------------------------------------------------------------- preprocess
sub preprocess {
    my ($t) = @_;
    $t =~ s{/\*.*?\*/}{}gs;
    $t =~ s{//[^\n]*}{}g;
    my @out;
    my @stack;    # each: [active_before, cond_value, in_else]
    my $active = 1;
    for my $line (split /\n/, $t) {
        if ($line =~ /^\s*#\s*ifdef\s+(\w+)/) { push @stack, [$active, 0, 0]; $active = 0; next; }
        if ($line =~ /^\s*#\s*ifndef\s+(\w+)/) { push @stack, [$active, 1, 0]; $active = $active && 1; next; }
        if ($line =~ /^\s*#\s*if\b/) { die "unhandled #if: $line"; }
        if ($line =~ /^\s*#\s*else/) {
            my $s = $stack[-1];
            $active = $s->[0] && !$s->[1];
            $s->[1] = !$s->[1];
            next;
        }
        if ($line =~ /^\s*#\s*endif/) { my $s = pop @stack; $active = $s->[0]; next; }
        next if $line =~ /^\s*#\s*(pragma|include)/;
        if ($line =~ /^\s*#\s*define\s+(\w+)\s+(\d+)\s*$/) { push @out, "// DEFINE $1 $2" if $active; next; }
        next if $line =~ /^\s*#/;
        push @out, $line if $active;
    }
    return join("\n", @out);
}

my $src = preprocess(slurp($packet_h));
my $gt = preprocess(slurp('client/include/Client/UI/GameType.h'));

# ---------------------------------------------------------------- constants
my %const = (
    CHARACTER_NAME_MAX_LEN => 24,    # client/include/Client/UI/StdAfx.h
    POINT_MAX_NUM => 255,            # StdAfx.h
    DS_REFINE_WINDOW_MAX_NUM => 15,  # GameType.h
    ITEM_SOCKET_SLOT_MAX_NUM => 3,   # GameType.h
    ITEM_ATTRIBUTE_SLOT_MAX_NUM => 7,
);
while ($src =~ /^\/\/ DEFINE (\w+) (\d+)/mg) { $const{$1} = $2; }

# enums: "NAME = expr," lists, and bare names counting up
my @enum_bodies;
while ($src =~ /\benum\b\s*\w*\s*\{([^{}]*)\}/g) { push @enum_bodies, $1; }

sub evalexpr {
    my ($e) = @_;
    $e =~ s/\s+//g;
    $e =~ s/\b([A-Za-z_]\w*)\b/exists $const{$1} ? $const{$1} : die("unknown constant $1 in '$_[0]'")/ge;
    $e =~ s/(0x[0-9a-fA-F]+)[uUlL]*/hex($1)/ge;
    $e =~ s/(\d+)[uUlL]+\b/$1/g;
    die "bad expr $_[0]" unless $e =~ /^[\d+\-*\/()<>|]+$/;
    return eval $e;
}

for my $body (@enum_bodies) {
    my $next = 0;
    for my $item (split /,/, $body) {
        $item =~ s/^\s+|\s+$//g;
        next unless length $item;
        if ($item =~ /^(\w+)\s*=\s*(.+)$/) { $const{$1} = evalexpr($2); $next = $const{$1} + 1; }
        elsif ($item =~ /^(\w+)$/) { $const{$1} = $next++; }
    }
}

# ---------------------------------------------------------------- header consts
my (@cg, @gc);
{
    my ($first) = $src =~ /\benum\b\s*\{(\s*HEADER_CG_LOGIN.*?)\}\s*;/s or die "header enum";
    for my $item (split /,/, $first) {
        $item =~ s/^\s+|\s+$//g;
        next unless $item =~ /^HEADER_(CG|GC)_(\w+)\s*=\s*(\S+)$/;
        my ($dir, $n, $v) = ($1, $2, $3);
        $v = hex($v) if $v =~ /^0x/i;
        push @{ $dir eq 'CG' ? \@cg : \@gc }, [$n, $v];
    }
}

# ---------------------------------------------------------------- structs
my %struct;      # canonical name -> { fields => [...], tag => ..., order => n }
my %alias;       # any name -> canonical name
my @order;

sub register_struct {
    my ($tag, $alias, $body) = @_;
    my $canon = $alias // $tag;
    die "anonymous struct without alias" unless defined $canon;
    my @fields;
    my $text = $body;
    $text =~ s/\{\s*\}//g;     # default member initialisers "{}"
    for my $decl (split /;/, $text) {
        $decl =~ s/^\s+|\s+$//g;
        next unless length $decl;
        next if $decl =~ /\)\s*(:|\{|$)/;    # constructor / method
        next if $decl =~ /^[A-Za-z_]\w*\s*\(/;
        $decl =~ s/\s*=\s*[^,]+$//;          # "= 0U"
        $decl =~ s/^struct\s+//;
        if ($decl =~ /^(.*?)\s+(\w+(?:\s*,\s*\w+)*)\s*((?:\[[^\]]*\])*)$/ && $2 !~ /\[/) {
            my ($type, $names, $arr) = ($1, $2, $3);
            $type =~ s/^struct\s+//;
            for my $n (split /\s*,\s*/, $names) {
                my @dims;
                while ($arr =~ /\[([^\]]*)\]/g) { push @dims, evalexpr($1); }
                push @fields, { name => $n, type => $type, dims => \@dims };
            }
        } else { die "cannot parse field '$decl' in $canon"; }
    }
    $struct{$canon} = { fields => \@fields, tag => $tag };
    push @order, $canon;
    $alias{$canon} = $canon;
    $alias{$tag} = $canon if defined $tag;
}

{
    my $t = $src;
    $t =~ s/\benum\b\s*\w*\s*\{[^{}]*\}\s*;//g;
    $t =~ s/\w\s*\{\s*\}\s*;/;/g if 0;
    $t =~ s/\]\s*\{\s*\}\s*;/];/g;
    $t =~ s/\b\w+\s*\(\)\s*:\s*[^{;]*\{\s*\}//g;
    # the one nested typedef (packet_shop_start_ex::TSubPacketShopTab)
    $t =~ s/typedef struct sub_packet_shop_tab\s*\{(.*?)\}\s*TSubPacketShopTab;/
        register_struct('sub_packet_shop_tab', 'TSubPacketShopTab', $1); ''/se;
    # innermost struct bodies, in source order
    while ($t =~ /(?:using\s+(\w+)\s*=\s*)?(?:typedef\s+)?struct\s*(\w+)?\s*\{([^{}]*)\}\s*(\w+)?\s*;/g) {
        my ($using, $tag, $body, $tdname) = ($1, $2, $3, $4);
        next if defined $tag && $tag eq '_AHNHS_TRANS_BUFFER';
        register_struct($tag, $using // $tdname, $body);
    }
}

# GameType.h structs that packets embed (TItemPos has methods; spelled out)
register_struct('SItemPos', 'TItemPos', "BYTE window_type; WORD cell;");
register_struct('SQuickSlot', 'TQuickSlot', "BYTE Type; BYTE Position;");
register_struct('TPlayerItemAttribute', 'TPlayerItemAttribute', "BYTE bType; short sValue;");
register_struct('packet_shop_item', 'TShopItemData',
    "DWORD vnum; DWORD price; BYTE count; BYTE display_pos; LONG alSockets[3]; TPlayerItemAttribute aAttr[7];");
# TPixelPosition = D3DXVECTOR3 (client/include/Client/Game/MapType.h): 3 floats
register_struct('D3DXVECTOR3', 'TPixelPosition', "float x; float y; float z;");

# ---------------------------------------------------------------- types
my %prim = (
    BYTE => ['u8', 1], 'unsigned char' => ['u8', 1], char => ['u8', 1],
    WORD => ['u16', 2], 'unsigned short' => ['u16', 2], short => ['i16', 2],
    DWORD => ['u32', 4], 'unsigned int' => ['u32', 4], LONG => ['i32', 4], int => ['i32', 4],
    float => ['f32', 4], bool => ['bool', 1],
    time_t => ['i64', 8],    # 64-bit on the Linux build; see the crate docs
);

sub resolve {
    my ($type) = @_;
    return { prim => $prim{$type} } if $prim{$type};
    my $c = $alias{$type};
    die "unknown type '$type'" unless defined $c;
    return { struct => $c };
}

my %size;
sub struct_size {
    my ($n) = @_;
    return $size{$n} if defined $size{$n};
    my $sz = 0;
    for my $f (@{ $struct{$n}{fields} }) {
        $sz += field_size($f);
    }
    return $size{$n} = $sz;
}
sub field_size {
    my ($f) = @_;
    my $r = resolve($f->{type});
    my $one = $r->{prim} ? $r->{prim}[1] : struct_size($r->{struct});
    my $n = 1;
    $n *= $_ for @{ $f->{dims} };
    return $one * $n;
}

sub rust_ty {
    my ($f) = @_;
    my $r = resolve($f->{type});
    my $t = $r->{prim} ? $r->{prim}[0] : $r->{struct};
    for my $d (reverse @{ $f->{dims} }) { $t = "[$t; $d]"; }
    return $t;
}

my %kw = map { $_ => 1 } qw(type mod fn match ref self in as loop move box where use);
sub rname { my ($n) = @_; return $kw{$n} ? "r#$n" : $n; }

# ---------------------------------------------------------------- header map
my @hmap;    # [header_name, struct, dynamic]
{
    my $cpp = slurp($stream_cpp);
    my ($cls) = $cpp =~ /(class CMainPacketHeaderMap.*?\n\};)/s or die "header map class";
    while ($cls =~ /Set\(\s*HEADER_GC_(\w+)\s*,\s*CNetworkPacketHeaderMap::TPacketType\(\s*sizeof\((\w+)\)\s*,\s*(STATIC|DYNAMIC)_SIZE_PACKET\s*\)\s*\)\s*;/gs) {
        push @hmap, [$1, $2, $3 eq 'DYNAMIC'];
    }
    # verbatim lines for the C++ dumper
    my @verbatim;
    my $body = $cls;
    $body =~ s{/\*.*?\*/}{}gs;
    $body =~ s{//[^\n]*}{}g;
    while ($body =~ /(Set\(\s*HEADER_GC_\w+[^;]*;)/g) {
        my $s = $1;
        $s =~ s/\s+/ /g;
        push @verbatim, $s;
    }
    die "header map count mismatch" unless @verbatim == @hmap;
    $main::verbatim = \@verbatim;
}
for my $h (@hmap) { die "no struct $h->[1]" unless $alias{ $h->[1] }; }

# CG: struct <-> header, from the senders (`var.header = HEADER_CG_X` after `TPacketCGY var;`)
my %cg_hdr;    # struct -> [HEADER names]
{
    for my $file (glob('client/src/ui/*.cpp')) {
        my %decl;
        for my $line (split /\n/, slurp($file)) {
            if ($line =~ /^\s*(TPacketCG\w+)\s+(\w+)\s*(?:[;=({]|$)/) { $decl{$2} = $1; }
            if ($line =~ /\b(\w+)\.\w*[hH]eader\s*=\s*HEADER_CG_(\w+)/ && $decl{$1}) {
                my $s = $decl{$1};
                push @{ $cg_hdr{$s} }, $2 unless grep { $_ eq $2 } @{ $cg_hdr{$s} || [] };
            }
        }
    }
}

# ---------------------------------------------------------------- emit Rust
sub camel { join '', map { ucfirst lc } split /_/, $_[0]; }

my $rs = <<'HDR';
// @generated by tools/differential/net/gen_packets.pl from
// client/include/Client/UI/Packet.h and client/src/ui/PythonNetworkStream.cpp.
// Do not edit by hand; edit the generator and re-run it.
//
// Every struct is the `#pragma pack(1)` wire layout of the C++ struct of the
// same name; field names are the C++ ones.  `char name[N]` fields are `[u8; N]`
// with `name_str()` / `set_name_str()` UTF-8 accessors (D14).
#![allow(non_snake_case, non_camel_case_types, clippy::all, dead_code)]

use super::{Reader, Wire, Writer, DecodeError, cstr_to_string, set_cstr};

HDR

$rs .= "/// `HEADER_CG_*` (client to server), from the first enum of Packet.h.\npub mod cg {\n";
$rs .= "    pub const $_->[0]: u8 = $_->[1];\n" for @cg;
$rs .= "    pub const ALL: &[(&str, u8)] = &[\n";
$rs .= "        (\"$_->[0]\", $_->[1]),\n" for @cg;
$rs .= "    ];\n}\n\n";
$rs .= "/// `HEADER_GC_*` (server to client).\npub mod gc {\n";
$rs .= "    pub const $_->[0]: u8 = $_->[1];\n" for @gc;
$rs .= "    pub const ALL: &[(&str, u8)] = &[\n";
$rs .= "        (\"$_->[0]\", $_->[1]),\n" for @gc;
$rs .= "    ];\n}\n\n";

# constants other code uses
$rs .= "/// Array-size constants of Packet.h that the state machine uses.\npub mod consts {\n";
for my $c (qw(ID_MAX_NUM PASS_MAX_NUM CHAT_MAX_NUM PLAYER_PER_ACCOUNT3 PLAYER_PER_ACCOUNT4 CHARACTER_NAME_MAX_LEN
    QUICKSLOT_MAX_NUM SHOP_HOST_ITEM_MAX_NUM METIN_SOCKET_COUNT PRIVATE_CODE_LENGTH WEAR_MAX_NUM SKILL_MAX_NUM
    CHR_EQUIPPART_NUM GUILD_NAME_MAX_LEN)) {
    next unless exists $const{$c};
    $rs .= "    pub const $c: usize = $const{$c};\n";
}
$rs .= "}\n\n";

my @layout_rows;
for my $n (@order) {
    my $s = $struct{$n};
    my @f = @{ $s->{fields} };
    $rs .= "#[derive(Clone, Copy, Debug, PartialEq)]\npub struct $n {\n";
    for my $f (@f) { $rs .= "    pub " . rname($f->{name}) . ": " . rust_ty($f) . ",\n"; }
    $rs .= "}\n\n";
    $rs .= "impl Wire for $n {\n";
    $rs .= "    const SIZE: usize = " . (@f ? join(' + ', map { "<" . rust_ty($_) . " as Wire>::SIZE" } @f) : "0") . ";\n";
    $rs .= "    fn write(&self, w: &mut Writer) {\n";
    $rs .= "        self." . rname($_->{name}) . ".write(w);\n" for @f;
    $rs .= "    }\n    fn read(r: &mut Reader) -> Result<Self, DecodeError> {\n        Ok(Self {\n";
    $rs .= "            " . rname($_->{name}) . ": Wire::read(r)?,\n" for @f;
    $rs .= "        })\n    }\n    fn zeroed() -> Self {\n        Self {\n";
    $rs .= "            " . rname($_->{name}) . ": Wire::zeroed(),\n" for @f;
    $rs .= "        }\n    }\n}\n\n";
    $rs .= "impl Default for $n {\n    fn default() -> Self {\n        Wire::zeroed()\n    }\n}\n\n";
    # constants and accessors
    my @acc;
    push @acc, "    /// `sizeof($n)` in the C++ build.\n    pub const SIZE: usize = <Self as Wire>::SIZE;\n";
    if ($cg_hdr{$n} && @{ $cg_hdr{$n} } == 1) {
        push @acc, "    /// Header the C++ client sets when it sends this packet.\n    pub const HEADER: u8 = cg::$cg_hdr{$n}[0];\n";
    }
    for my $f (@f) {
        my ($r, $d) = (resolve($f->{type}), $f->{dims});
        if ($r->{prim} && $f->{type} eq 'char' && @$d == 1) {
            my $nm = $f->{name};
            my $rn = rname($nm);
            (my $base = $nm) =~ s/^r#//;
            push @acc, "    /// UTF-8 view of `$nm` (up to the first NUL; invalid bytes become U+FFFD).\n    pub fn ${base}_str(&self) -> String {\n        cstr_to_string(&self.$rn)\n    }\n";
            push @acc, "    /// `strncpy` into `$nm`, always leaving a trailing NUL.\n    pub fn set_${base}_str(&mut self, s: &str) {\n        set_cstr(&mut self.$rn, s);\n    }\n";
        }
    }
    $rs .= "impl $n {\n" . join("\n", @acc) . "}\n\n";

    my $off = 0;
    my @offs;
    for my $f (@f) { push @offs, [$f->{name}, $off]; $off += field_size($f); }
    push @layout_rows, [$n, $off, \@offs];
}

# layout table computed by Rust itself from Wire::SIZE (independent of the generator's arithmetic)
$rs .= "/// Name, `Wire::SIZE` and field offsets (running sum of the field `Wire::SIZE`s) of every struct.\n";
$rs .= "pub fn layouts() -> Vec<(&'static str, usize, Vec<(&'static str, usize)>)> {\n    let mut v = Vec::new();\n";
for my $n (@order) {
    my @f = @{ $struct{$n}{fields} };
    $rs .= "    {\n        let mut off = 0usize;\n        let mut f = Vec::new();\n";
    for my $x (@f) {
        $rs .= "        f.push((\"$x->{name}\", off));\n        off += <" . rust_ty($x) . " as Wire>::SIZE;\n";
    }
    $rs .= "        v.push((\"$n\", off, f));\n    }\n";
}
$rs .= "    v\n}\n\n";

# header map
$rs .= "/// One entry of `CMainPacketHeaderMap` (PythonNetworkStream.cpp): the size of the\n/// struct and whether the packet is dynamic-size (header, u16 size, ...).\n";
$rs .= "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct HeaderInfo {\n    pub header: u8,\n    pub name: &'static str,\n    pub size: usize,\n    pub dynamic: bool,\n}\n\n";
$rs .= "/// `CMainPacketHeaderMap`, in the C++ registration order.\npub const GC_HEADER_MAP: &[HeaderInfo] = &[\n";
for my $h (@hmap) {
    $rs .= "    HeaderInfo { header: gc::$h->[0], name: \"$h->[0]\", size: <$alias{$h->[1]} as Wire>::SIZE, dynamic: " . ($h->[2] ? 'true' : 'false') . " },\n";
}
$rs .= "];\n\n";

# GcPacket enum
my %seen_variant;
$rs .= "/// A decoded server packet: one variant per entry of the header map (in map order).\n";
$rs .= "/// Dynamic-size packets carry the bytes after the fixed struct in `tail`.\n";
$rs .= "#[derive(Clone, Debug, PartialEq)]\npub enum GcPacket {\n";
my @variants;
for my $h (@hmap) {
    my $v = camel($h->[0]);
    die "dup variant $v" if $seen_variant{$v}++;
    my $st = $alias{ $h->[1] };
    push @variants, [$h->[0], $v, $st, $h->[2]];
    $rs .= $h->[2] ? "    $v { head: $st, tail: Vec<u8> },\n" : "    $v($st),\n";
}
$rs .= "}\n\nimpl GcPacket {\n";
$rs .= "    /// Decodes one complete frame (header byte first) according to the header map.\n";
$rs .= "    pub fn decode(frame: &[u8]) -> Result<GcPacket, DecodeError> {\n";
$rs .= "        let header = *frame.first().ok_or(DecodeError::Truncated)?;\n        match header {\n";
for my $v (@variants) {
    my ($hn, $vn, $st, $dyn) = @$v;
    if ($dyn) {
        $rs .= "            gc::$hn => {\n                let head = $st\::decode(frame)?;\n                Ok(GcPacket::$vn { head, tail: frame[<$st as Wire>::SIZE..].to_vec() })\n            }\n";
    } else {
        $rs .= "            gc::$hn => Ok(GcPacket::$vn($st\::decode(frame)?)),\n";
    }
}
$rs .= "            h => Err(DecodeError::UnknownHeader(h)),\n        }\n    }\n\n";
$rs .= "    /// Re-encodes (inverse of `decode`).\n    pub fn encode(&self) -> Vec<u8> {\n        match self {\n";
for my $v (@variants) {
    my ($hn, $vn, $st, $dyn) = @$v;
    if ($dyn) {
        $rs .= "            GcPacket::$vn { head, tail } => {\n                let mut v = head.encode();\n                v.extend_from_slice(tail);\n                v\n            }\n";
    } else {
        $rs .= "            GcPacket::$vn(p) => p.encode(),\n";
    }
}
$rs .= "        }\n    }\n\n    /// The `HEADER_GC_*` value of this packet.\n    pub fn header(&self) -> u8 {\n        match self {\n";
for my $v (@variants) {
    my ($hn, $vn, $st, $dyn) = @$v;
    $rs .= "            GcPacket::$vn" . ($dyn ? " { .. }" : "(_)") . " => gc::$hn,\n";
}
$rs .= "        }\n    }\n}\n";

# CG headers table with structs, for tests
$rs .= "\n/// `HEADER_CG_*` -> struct, for the senders found in client/src/ui (name, struct, size).\npub fn cg_senders() -> Vec<(&'static str, &'static str, usize)> {\n    vec![\n";
for my $s (sort keys %cg_hdr) {
    for my $h (@{ $cg_hdr{$s} }) {
        $rs .= "        (\"$h\", \"$s\", <$s as Wire>::SIZE),\n";
    }
}
$rs .= "    ]\n}\n";

open my $o, '>', $out_rs or die "$out_rs: $!";
print $o $rs;
close $o;

# ---------------------------------------------------------------- emit .inc
my $inc = "// \@generated by tools/differential/net/gen_packets.pl - do not edit.\n";
$inc .= "// STRUCT(Type) opens a struct, FIELD(Type, field) prints sizeof/offsetof.\n";
for my $n (@order) {
    next unless $struct{$n}{tag} || 1;
    my $cn = $n eq "TSubPacketShopTab" ? "TPacketGCShopStartEx::TSubPacketShopTab" : $n;
    # use the C++ name that Packet.h/GameType.h declares
    $inc .= "STRUCT($cn)\n";
    $inc .= "  FIELD($cn, $_->{name})\n" for @{ $struct{$n}{fields} };
    $inc .= "END_STRUCT\n";
}
$inc .= "// header constants\n";
$inc .= "HDR(CG, $_->[0])\n" for @cg;
$inc .= "HDR(GC, $_->[0])\n" for @gc;
$inc .= "// verbatim from CMainPacketHeaderMap\n";
$inc .= "$_\n" for @{ $main::verbatim };
open my $o2, '>', $out_inc or die "$out_inc: $!";
print $o2 $inc;
close $o2;

printf STDERR "%d structs, %d cg headers, %d gc headers, %d map entries, %d cg senders\n",
    scalar(@order), scalar(@cg), scalar(@gc), scalar(@hmap), scalar(keys %cg_hdr);
