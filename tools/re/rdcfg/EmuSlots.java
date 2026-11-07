// Headless: run RDCfg's button writer FUN_0045c570(profile 0, slot, slot) for each slot of a
// product in Ghidra's PCode emulator (same call interception as EmuModels.java) and log the
// feature reports it sends: the write address gives the slot's offset in the profile block.
// Used for single-model RDCfg builds, whose slot list lives in Default.bin (tools/re/fleet).
// args: outFile model slot[:ui][,...]   (ui defaults to slot; env EMU_MAP: see EmuModels.java)
import ghidra.app.emulator.EmulatorHelper;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.lang.Register;
import java.io.*;
import java.math.BigInteger;
import java.util.*;

public class EmuSlots extends GhidraScript {
    static final long OBJ = 0x10000000L, STACK = 0x0ff00000L, RET = 0x0dead000L, NAMEQ = 0x0ee00000L;
    static final Set<String> RUN = new HashSet<>(Arrays.asList("FUN_00431a30", "FUN_004314d0", "FUN_0045c570"));

    String model;
    Map<String, Long> arg1 = new HashMap<>();
    PrintWriter out;
    // another RDCfg build: env EMU_MAP = file of "REF NEW" names (tools/re/fleet/resolve_rdcfg.py);
    // callees are reported under their reference name, entry points/data translated to this build
    static Map<String, String> toNew = new HashMap<>(), toRef = new HashMap<>();
    static {
        String mp = System.getenv("EMU_MAP");
        if (mp != null) try (BufferedReader r = new BufferedReader(new FileReader(mp))) {
            for (String l; (l = r.readLine()) != null; ) {
                String[] p = l.trim().split(" ");
                if (p.length == 2) { toNew.put(p[0], p[1]); toRef.put(p[1], p[0]); }
            }
        } catch (IOException e) { throw new RuntimeException(e); }
    }
    // env EMU_STUB = "FUN_x=hex,...": functions (this build's names) that just return a value,
    // e.g. single-model builds' Default.bin getters (QSettings is not emulated)
    static Map<String, Long> stub = new HashMap<>();
    static {
        String st = System.getenv("EMU_STUB");
        if (st != null) for (String kv : st.split(",")) {
            String[] p = kv.split("=");
            stub.put(p[0], Long.parseLong(p[1], 16));
        }
    }
    static long addrOf(String refName, long refAddr) {
        String n = toNew.get(refName);
        return n == null ? refAddr : Long.parseLong(n.replaceAll("^(FUN|DAT)_", ""), 16);
    }

    String calleeName(Instruction ins) {
        String n0 = calleeName0(ins);
        return toRef.getOrDefault(n0, n0);
    }

    String calleeName0(Instruction ins) {
        Address t = null;
        Address[] fl = ins.getFlows();
        if (fl != null && fl.length > 0) t = fl[0];
        if (t == null) {
            for (Reference r : ins.getReferencesFrom()) {
                if (r.getReferenceType().isCall() || r.getReferenceType().isData()) {
                    Symbol s = getSymbolAt(r.getToAddress());
                    if (s != null && s.isExternal()) return s.getName(true);
                    Data d = getDataAt(r.getToAddress());
                    if (d != null) {
                        Reference[] rr = getReferencesFrom(r.getToAddress());
                        for (Reference x : rr) {
                            Symbol es = getSymbolAt(x.getToAddress());
                            if (es != null) return es.getName(true);
                        }
                    }
                    if (s != null) return s.getName(true);
                }
            }
            return "?";
        }
        Function f = getFunctionAt(t);
        if (f == null) return "?" + t;
        if (f.isThunk()) {
            Function tf = f.getThunkedFunction(true);
            if (tf != null) return tf.getName(true);
        }
        return f.getName();
    }

    long u32(EmulatorHelper e, long a) throws Exception {
        byte[] b = e.readMemory(toAddr(a), 4);
        return (b[0] & 0xffL) | (b[1] & 0xffL) << 8 | (b[2] & 0xffL) << 16 | (b[3] & 0xffL) << 24;
    }

    String cstr(EmulatorHelper e, long a) {
        try {
            if (a < 0x400000L || a > 0x600000L) return null;
            byte[] b = e.readMemory(toAddr(a), 64);
            int n = 0;
            while (n < b.length && b[n] >= 0x20 && b[n] < 0x7f) n++;
            if (n < 2 || n == b.length || b[n] != 0) return null;
            return new String(b, 0, n);
        } catch (Exception x) { return null; }
    }

    long[] stackArgs = null; long[][] pokes = new long[0][];
    void runOne(Address entry) throws Exception {
        EmulatorHelper e = new EmulatorHelper(currentProgram);
        try {
            Register esp = e.getStackPointerRegister();
            e.writeMemory(toAddr(OBJ), new byte[0x4000]);
            for (long k = 0; k < 0x4000; k += 4) {
                long v = 0x20000000L + k;
                e.writeMemory(toAddr(OBJ + k), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
            }
            // double 1.0 at this+0x30 (UI scale)
            e.writeMemory(toAddr(OBJ + 0x30), new byte[]{0, 0, 0, 0, 0, 0, (byte) 0xf0, 0x3f});
            e.writeMemory(toAddr(NAMEQ), new byte[64]);
            e.writeMemory(toAddr(STACK - 0x10000), new byte[0x10010]);
            long sp = STACK - 0x100;
            e.writeMemory(toAddr(sp), new byte[]{(byte) 0x00, (byte) 0xd0, (byte) 0xea, 0x0d});
            for (int k = 1; k < 8; k++) {
                long v = OBJ; // stack params also point at the fake object
                e.writeMemory(toAddr(sp + 4 * k), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
            }
            if (stackArgs != null) for (int k = 0; k < stackArgs.length; k++) {
                long v = stackArgs[k];
                e.writeMemory(toAddr(sp + 4 + 4 * k), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
            }
            for (long[] pk : pokes) {
                long v = pk[1];
                e.writeMemory(toAddr(pk[0]), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
            }
            if (arg1.containsKey(model)) {
                long v = arg1.get(model);
                e.writeMemory(toAddr(sp + 4), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
            }
            e.writeRegister(esp, sp);
            e.writeRegister("ECX", OBJ);
            e.writeRegister("EDX", OBJ);
            e.writeRegister(e.getPCRegister(), entry.getOffset());
            int steps = 0;
            while (!monitor.isCancelled() && steps++ < 400000) {
                Address pc = e.getExecutionAddress();
                if (pc.getOffset() == RET) { out.println(model + "\tRET\t" + Long.toHexString(e.readRegister("EAX").longValue())); break; }
                Instruction ins = getInstructionAt(pc);
                if (ins == null) { out.println(model + "\tERR\tno instruction at " + pc); return; }
                if (ins.getFlowType().isCall()) {
                    String raw = calleeName0(ins);
                    String name = toRef.getOrDefault(raw, raw);
                    long s = e.readRegister(esp).longValue();
                    long[] a = new long[8];
                    for (int k = 0; k < 8; k++) a[k] = u32(e, s + 4 * k);
                    Address next = ins.getFallThrough();
                    if (RUN.contains(name) && !stub.containsKey(raw)) {
                        StringBuilder rb = new StringBuilder(model + "\t" + pc + "\t" + name + "\t" + Long.toHexString(e.readRegister("ECX").longValue()));
                        for (int k = 0; k < 8; k++) rb.append('\t').append(Long.toHexString(a[k]));
                        out.println(rb);
                        // real call: push return address, jump
                        long ns = s - 4;
                        long r = next.getOffset();
                        e.writeMemory(toAddr(ns), new byte[]{(byte) r, (byte) (r >> 8), (byte) (r >> 16), (byte) (r >> 24)});
                        e.writeRegister(esp, ns);
                        Address[] fl = ins.getFlows();
                        e.writeRegister(e.getPCRegister(), fl[0].getOffset());
                        continue;
                    }
                    long ret = 0;
                    if (name.equals("FUN_004a24b0") || name.equals("FUN_004a24f0")) {
                        String c = cstr(e, a[0]);
                        if (c == null) c = cstr(e, a[1]);
                        ret = (c != null && c.equals(model)) ? 1 : 0;
                    } else if (name.contains("compare_helper")) {
                        String c = null;
                        for (int k = 0; k < 4 && c == null; k++) c = cstr(e, a[k]);
                        ret = (c != null && c.equals(model)) ? 0 : 1;
                    } else if (name.equals("FUN_00423c30")) {
                        // returns the current product name QString into *arg0
                        long dst = a[0];
                        long v = NAMEQ;
                        e.writeMemory(toAddr(dst), new byte[]{(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
                        ret = dst;
                    }
                    if (name.equals("load") || name.endsWith("::load")) ret = 1;
                    if (stub.containsKey(raw)) ret = stub.get(raw);
                    StringBuilder sb = new StringBuilder();
                    sb.append(model).append('\t').append(pc).append('\t').append(name).append('\t')
                      .append(Long.toHexString(e.readRegister("ECX").longValue()));
                    for (int k = 0; k < 8; k++) {
                        sb.append('\t').append(Long.toHexString(a[k]));
                        String c = cstr(e, a[k]);
                        if (c != null) sb.append("=\"").append(c).append('"');
                    }
                    // QRect* arguments: dump 4 ints
                    if (name.contains("setGeometry")) {
                        long q = a[0];
                        sb.append("\tRECT");
                        for (int k = 0; k < 4; k++) sb.append(' ').append((int) u32(e, q + 4 * k));
                    }
                    if (name.contains("hid_send_feature_report")) {
                        sb.append("\tBUF");
                        byte[] b = e.readMemory(toAddr(a[1]), (int) Math.min(a[2], 256));
                        for (byte x : b) sb.append(String.format(" %02x", x & 0xff));
                    }
                    out.println(sb);
                    // callee-pop: GCC re-adjusts with "sub esp, N" right after
                    long purge = 0;
                    Instruction nx = getInstructionAt(next);
                    if (nx != null && nx.getMnemonicString().equals("SUB") && nx.getNumOperands() == 2
                            && nx.getDefaultOperandRepresentation(0).equals("ESP")) {
                        Object[] o = nx.getOpObjects(1);
                        if (o.length == 1 && o[0] instanceof ghidra.program.model.scalar.Scalar)
                            purge = ((ghidra.program.model.scalar.Scalar) o[0]).getUnsignedValue();
                    }
                    e.writeRegister("EAX", ret);
                    e.writeRegister(esp, s + purge);
                    e.writeRegister(e.getPCRegister(), next.getOffset());
                    continue;
                }
                if (!e.step(monitor)) { out.println(model + "\tERR\t" + e.getLastError() + " at " + pc); return; }
            }
            if (steps >= 400000) out.println(model + "\tERR\tstep limit");
        } finally {
            e.dispose();
        }
    }

    @Override
    public void run() throws Exception {
        // args: outFile model slot[,slot...]   (button writer FUN_0045c570(profile, slot, ui) per slot)
        String[] args = getScriptArgs();
        out = new PrintWriter(new FileWriter(args[0]));
        model = args[1];
        try {
            for (String sl : args[2].split(",")) {
                String[] su = sl.split(":");
                long slot = Long.parseLong(su[0]), ui = su.length > 1 ? Long.parseLong(su[1]) : slot;
                out.println(model + "\tSLOT\t" + slot);
                stackArgs = new long[]{0, slot, ui};
                pokes = new long[0][];
                runOne(toAddr(addrOf("FUN_0045c570", 0x45c570L)));
            }
        } finally { out.close(); }
    }
}
