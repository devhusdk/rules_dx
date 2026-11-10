-- | Converts one hpc .tix file and its .mix directories into LCOV.
--
-- Usage: hpc-lcov OUTPUT TIX MIXDIR...
module Main (main) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import System.Environment (getArgs)
import System.Exit (die)
import Trace.Hpc.Mix (BoxLabel (ExpBox), Mix (Mix), readMix)
import Trace.Hpc.Tix (Tix (Tix), TixModule, readTix, tixModuleTixs)
import Trace.Hpc.Util (fromHpcPos)

type LineHits = Map Int Integer

main :: IO ()
main = do
  args <- getArgs
  case args of
    output : tixPath : mixDirs@(_ : _) -> do
      Tix modules <- readTix tixPath >>= maybe (die ("cannot read " ++ tixPath)) pure
      files <- mapM (moduleHits mixDirs) modules
      writeFile output (render (Map.unionsWith (Map.unionWith max) files))
    _ -> die "usage: hpc-lcov OUTPUT TIX MIXDIR..."

moduleHits :: [FilePath] -> TixModule -> IO (Map FilePath LineHits)
moduleHits mixDirs tixModule = do
  Mix source _ _ _ entries <- readMix mixDirs (Right tixModule)
  let hits = [(line, count) | (count, (pos, ExpBox _)) <- zip (tixModuleTixs tixModule) entries, let (line, _, _, _) = fromHpcPos pos]
  pure (Map.singleton source (Map.fromListWith max hits))

render :: Map FilePath LineHits -> String
render files = concatMap record (Map.toList files)
  where
    record (source, hits) =
      unlines $
        ["TN:", "SF:" ++ source]
          ++ [concat ["DA:", show line, ",", show count] | (line, count) <- Map.toList hits]
          ++ ["LF:" ++ show (Map.size hits), "LH:" ++ show (Map.size (Map.filter (> 0) hits)), "end_of_record"]
