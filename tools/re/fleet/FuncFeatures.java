// Headless: dump per-function features used to match functions between two builds
// of the same program (tools/re/fleet/match_funcs.py).
// Output TSV: entry, ninstr, mnemonic hash, callees (external names or FUN_ addrs),
// scalar constants, referenced strings.
// args: outFile
import ghidra.app.script.GhidraScript;
import ghidra.app.plugin.match.ExactMnemonicsFunctionHasher;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class FuncFeatures extends GhidraScript {
    String callee(Instruction ins) {
        for (Reference r : ins.getReferencesFrom()) {
            if (!r.getReferenceType().isCall() && !r.getReferenceType().isData()) continue;
            Address t = r.getToAddress();
            Function f = getFunctionAt(t);
            if (f != null) {
                if (f.isThunk() && f.getThunkedFunction(true) != null) f = f.getThunkedFunction(true);
                return f.isExternal() || f.getName().startsWith("FUN_") ? f.getName() : f.getName(true);
            }
            Symbol s = getSymbolAt(t);
            if (s != null && s.isExternal()) return s.getName(true);
            for (Reference x : getReferencesFrom(t)) {
                Symbol es = getSymbolAt(x.getToAddress());
                if (es != null && es.isExternal()) return es.getName(true);
            }
        }
        return null;
    }

    @Override
    public void run() throws Exception {
        PrintWriter out = new PrintWriter(new FileWriter(getScriptArgs()[0]));
        Listing L = currentProgram.getListing();
        for (Function f : L.getFunctions(true)) {
            if (monitor.isCancelled()) break;
            int n = 0;
            List<String> calls = new ArrayList<>();
            TreeSet<String> consts = new TreeSet<>(), strs = new TreeSet<>();
            for (Instruction ins : L.getInstructions(f.getBody(), true)) {
                n++;
                if (ins.getFlowType().isCall()) {
                    String c = callee(ins);
                    calls.add(c == null ? "?" : c);
                }
                for (int k = 0; k < ins.getNumOperands(); k++) {
                    for (Object o : ins.getOpObjects(k)) {
                        if (o instanceof Scalar) {
                            long v = ((Scalar) o).getUnsignedValue();
                            if (v >= 0x10 && (v < 0x400000L || v > 0x700000L) && v < 0x100000000L) consts.add(Long.toHexString(v));
                        }
                    }
                }
                for (Reference r : ins.getReferencesFrom()) {
                    Data d = getDataAt(r.getToAddress());
                    if (d != null && d.hasStringValue()) {
                        String s = String.valueOf(d.getValue()).replaceAll("[\\t\\n\\r]", " ");
                        if (s.length() > 60) s = s.substring(0, 60);
                        strs.add(s);
                    }
                }
            }
            long h = 0;
            try { h = ExactMnemonicsFunctionHasher.INSTANCE.hash(f, monitor); } catch (Exception e) { }
            out.println(f.getEntryPoint() + "\t" + n + "\t" + Long.toHexString(h) + "\t" + String.join(",", calls)
                + "\t" + String.join(",", consts) + "\t" + String.join("\u0001", strs));
        }
        out.close();
    }
}
